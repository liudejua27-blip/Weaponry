# ForgeCAD 架构决策索引

2026-08-15 最新实现覆盖：当前为 118 个 JSON Schema、37 read + 30 opt-in write = 67 tools。`repair_apply_confirm` 已加入单视图 Repair 的 source boundary；`design_action_optimization_proposal_prepare` 已加入 ActionRun→CADFit 的独立 review-candidate continuation，不自动 Repair/Confirm。Rust focused test 与 workspace test 已通过，多视图仍必须走 `cross_view_promotion_confirm`。

当前树保留 `ADR-0025-codex-only-mcp-3d-runtime.md` 和 `ADR-0026-agentic-design-runtime.md`。旧 ADR 已从产品树删除，历史查询依赖 reset archive/Git history，不得作为实施合同或能力证据。

ADR-0025 已于 2026-08-09 增补单用户 MVP profile：OS 文件锁、薄 MCP、可选 Viewer、MCP005–009 functional core、工具/Skill 目录与 MCP010–013 发布分界。该增补收窄实施复杂度，没有改变 Codex-only、Runtime 唯一写者、typed Operator 和无任意脚本的核心决策。

ADR-0026 于 2026-08-13 增补 Agentic Design Runtime 目标架构：在 ADR-0025 的 Runtime/MCP/Viewer 边界上，新增 DesignSession、SemanticSceneGraph、ReferenceCanvas、阶段质量门、Visual Evidence Bundle 和 Critic/Repair loop 的目标设计。observe/plan read-only projection、durable session/checkpoint/RepairIntent prepare/readback、bounded multi-view authoring/readback、single-action geometry ActionRun、hash-bound `CrossViewEvidenceBundle@1` boundary、approval-gated same-stage independent batch、`repair_apply_prepare` CAS-backed apply-intent boundary、单视图 `repair_apply_confirm` source boundary 和 `design_action_optimization_proposal_prepare` CADFit continuation 已实现；完整组合式 orchestrator、Repair runtime test、跨视图同 cohort conformance 和跨视图视觉闭环仍未完成，不改变当前 118 Schema、67 tools 或 MCP010F `QUALITY_TARGET_NOT_MET` 事实。

后续相反架构变更必须新增编号，说明替代范围、数据迁移、兼容性、许可证和退出条件。
