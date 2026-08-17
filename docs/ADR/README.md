# ForgeCAD 架构决策索引

当前树保留 `ADR-0025-codex-only-mcp-3d-runtime.md` 和 `ADR-0026-agentic-design-runtime.md`。旧 ADR 已从产品树删除，历史查询依赖 reset archive/Git history，不得作为实施合同或能力证据。

ADR-0025 已于 2026-08-09 增补单用户 MVP profile：OS 文件锁、薄 MCP、可选 Viewer、MCP005–009 functional core、工具/Skill 目录与 MCP010–013 发布分界。该增补收窄实施复杂度，没有改变 Codex-only、Runtime 唯一写者、typed Operator 和无任意脚本的核心决策。

ADR-0026 于 2026-08-13 增补 Agentic Design Runtime 目标架构：在 ADR-0025 的 Runtime/MCP/Viewer 边界上，新增 DesignSession、SemanticSceneGraph、ReferenceCanvas、阶段质量门、Visual Evidence Bundle 和 Critic/Repair loop 的目标设计。observe/plan read-only projection、durable session/checkpoint/RepairIntent prepare/readback 与窄范围 `repair_intent_run_prepare` source slice 已实现；单动作 orchestrator、Repair 应用和完整视觉闭环仍未完成。当前源码为 129 Schema、41 read + 33 opt-in write = 74 tools；旧 102 Schema/59 tools 仅保留为历史 cohort 快照，MCP010F 仍为 `QUALITY_TARGET_NOT_MET`。

后续相反架构变更必须新增编号，说明替代范围、数据迁移、兼容性、许可证和退出条件。
