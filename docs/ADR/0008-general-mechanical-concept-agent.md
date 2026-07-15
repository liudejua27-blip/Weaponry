# ADR-0008：通用机械概念 3D Agent 与首批四领域包

- 状态：Accepted
- 日期：2026-07-12
- 决策者：项目维护者
- 取代：ADR-0007 中“Weapon Concept Pack 是唯一第一阶段内容包”的决定

## 背景

Weapon Concept Pack 已证明模块、Connector、版本、质量和导出闭环，但产品目标已扩展：零基础用户应能描述未来武器道具、汽车、飞机、机械臂和其他机械创意，由同一个 Agent 生成完整外观方向，再继续分件、替换、调整比例和材质。

如果继续把 WeaponConceptSpec 作为产品级权威合同，其他领域会被迫套用武器语义；如果为每个领域复制工作台、Agent 和数据库，又会产生四套不一致系统。

## 决策

1. 产品定义升级为“通用机械概念 3D Agent”。
2. `future_weapon_prop`、`vehicle_concept`、`aircraft_concept` 和 `robotic_arm_concept` 是同级首批领域包。
3. 用户面对一个工作台、一个 Agent 和一个项目模型；领域包由 Agent 推断，不成为 Mode 页面。
4. 目标权威链为 `MechanicalConceptSpec → AssemblyGraph → ShapeProgram/registered modules → GLB`。
5. `AssemblyGraph` 支持分层部件、Connector、概念级 Joint、Material Zone 和来源追踪。
6. 每次新建设计先生成完整 `blockout`，再自动分件为 `segmented_concept`，最后整理为 `editable_asset`。
7. P0 材质是可追溯的视觉 PBR 预设；不以视觉材质推断真实强度、工艺或制造适用性。
8. 当前 Weapon Concept 数据和 API 通过兼容 adapter 迁移，不机械改名，也不冒充四领域已经实现。
9. CAD/DFM Engineering Pack 继续作为独立后续轨，不把 Mesh/ShapeProgram 冒充 B-Rep 工程 CAD。

## 后果

- README、DESIGN、IMPLEMENTATION_PLAN、CODEX_EXECUTION_PLAN、工作台和操作手册使用通用机械产品定义。
- 新增 DomainPackManifest、MechanicalConceptSpec、AssemblyGraph、Joint 和 MaterialPreset 目标合同。
- 固定评测必须覆盖四个领域包，而不是只测武器 Brief。
- 当前 weapon reference pack 保留为第一个兼容 fixture 和历史证据。
- 资产制作和命名规范逐步泛化，但现有稳定 ID、hash 和路径不重命名。

## 被否决方案

- 继续只做 Weapon Concept：无法满足通用机械产品定位。
- 为汽车、飞机和机械臂各建独立工作台：重复 UI、Agent、版本和导出系统。
- 直接让大模型输出不可编辑完整网格：无法稳定分件、替换和继续调整。
- 首版同时加入工程仿真：会把概念资产、视觉材质与工程结论混在一起。
