# ForgeCAD 当前架构决策

版本：2026-07-29
状态：当前决策索引

当前树只保留仍影响产品、运行时、资产真值或发布边界的 ADR。已经完全被取代的 ADR-0001–0008、0010–0012 和 0018 已从当前文档树清理；需要追溯时使用 Git 历史，不再让旧路线与当前产品并列出现。

## 当前决策

- [ADR-0009：ActiveDesignSnapshot 唯一状态合同](0009-active-design-snapshot.md)
- [ADR-0013：Manifold Python 唯一生产 CSG](0013-adopt-manifold-python-csg.md)
- [ADR-0014：Rust-first app-server 与受限几何执行器](0014-rust-first-codex-app-server.md)
- [ADR-0015：生产工件与真人视觉验收拆分](0015-split-production-artifact-and-visual-acceptance.md)
- [ADR-0016：Design Surface Compiler](0016-design-surface-compiler.md)
- [ADR-0017：三维设计工作区与视觉收敛](0017-codex-design-workspace-visual-convergence.md)
- [ADR-0019：程序化视觉默认 MVP](0019-programmatic-visual-program-mvp.md)
- [ADR-0020：小团队轻量、外观优先与成本纪律](0020-lightweight-appearance-first-3d-agent.md)
- [ADR-0021：高自由度 Visual Program 与 1+1](0021-high-freedom-visual-program-max.md)
- [ADR-0022：类别开放通用参考条件 3D Agent](0022-universal-reference-conditioned-3d-agent.md)
- [ADR-0023：DeepSeek / 千问唯一 AI Provider](0023-deepseek-qwen-only-ai-provider-policy.md)

ADR-0022 是最高产品范围决策；ADR-0023 是当前运行时 AI Provider 最高边界；ADR-0020/0021 只继续拥有轻量化、成本、typed program、一次 author + 最多一次 patch、单一真值和验收纪律。当前能力仍以 `USER_GUIDE.md` 和能力矩阵为准，目标 ADR 不能冒充实现。

## 维护规则

1. 新决策必须写明取代范围、实施任务、迁移和 Gate；
2. 已完全取代且不再参与当前架构的 ADR 从工作树删除，由 Git 保存历史；
3. 当前 ADR 不记录聊天进度、长命令输出或一次性截图；
4. 代码事实、任务状态和用户能力分别归 `DESIGN.md`、`CODEX_TASK_INDEX.md` 与 `USER_GUIDE.md`。
