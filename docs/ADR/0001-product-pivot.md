# ADR-0001：从武神 Forge 转向 ForgeCAD

- 状态：Accepted；第 6 项已由 ADR-0006 取代
- 日期：2026-07-10
- 决策者：项目维护者

## 背景

现有产品围绕幻想武器美术资产、Creative Recast、概念图、神经 3D 粗模和 Unity 交付构建。新目标是面向 3D 打印功能件的本地优先 AI 参数化 CAD / FDM DFM Agent。

两者可以复用桌面壳、FastAPI、SQLite、内容寻址资产、Job/Step/Event/SSE、幂等、恢复和追加式版本；但领域合同、几何真值、验证方式、输出格式和责任边界不兼容。

## 决策

1. 新产品临时代号为 ForgeCAD，正式品牌后置。
2. 保留通用基础设施，重建 CAD/DFM 领域内核。
3. 旧 `/api/weapons`、Weapon Schema、CreativeWeaponGraph、SkillGraph、神经 3D 主链和 Unity 导出立即进入功能冻结，仅允许迁移期缺陷修复。
4. 新 API 从 `/api/v1` 开始，新代码进入 `forgecad_agent` 及其明确子模块。
5. 旧数据库只读导入；不把 CreativeWeaponGraph 自动转换为 FeatureGraph，不长期双写新旧表。
6. 首版仅支持低风险、单零件、FDM 功能件，并拒绝武器和安全关键用途。
7. `legacy-wushen-v0.1` tag 是旧产品恢复点。

## 后果

- 旧 release gate 仍可用于历史证据，但不能证明 ForgeCAD 发布就绪。
- 新产品需要 C01–C10 合同、几何、DFM、沙箱和桌面 E2E 门禁。
- 旧领域最终删除，而不是永久维护兼容层。
- 迁移期间文档和 UI 必须显式区分 legacy 与 target，不能把规划能力写成已实现。

## 被否决方案

- 将 Weapon 重命名为 Design、Unity 重命名为 STEP：语义和安全边界不成立。
- 在旧主链旁长期双写 CAD 数据：会形成两个权威模型和不可控迁移成本。
- 先做新首页再补内核：不能验证产品核心价值。
