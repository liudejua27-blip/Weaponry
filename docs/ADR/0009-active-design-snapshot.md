# ADR-0009：ActiveDesignSnapshot 作为工作台唯一状态合同

- 状态：Accepted（合同、存储、API、desktop client/reducer、Agent 工作台核心接入、legacy 只读重建、不可变回退/前进和核心 CAS 竞争已完成；广泛多客户端压力仍待验证）
- 日期：2026-07-13
- 决策者：项目维护者
- 取代：任何由旧 Concept hook、Agent asset hook 和 localStorage 拼接“当前设计”的做法

## 背景

ForgeCAD 同时存在 legacy `ConceptVersion/ModuleGraph` 和通用机械 `AgentAssetVersion/AssemblyGraph`。它们各自有版本、选择、质量和导出路径；让前端自行组合会导致同一工作台显示两个版本或导出另一条版本链。

## 决策

1. 新增 `ActiveDesignSnapshot@1`，以 Project 为根、以 `revision` 为并发版本；
2. Snapshot 的 `active_design` 是判别联合：`agent_asset` 或 `legacy_concept_read_only`，不能同时存在两个活动版本；
3. 每个引用都携带 `project_id`，合同层拒绝跨 Project 引用；
4. Agent 状态下 preview 的 base、quality 的 asset、export 的 source version 必须等于 active Agent asset version；
5. legacy 只读状态不能带 Agent part selection、preview 或 quality，导出只能指向当前 legacy version；
6. S001 冻结 Schema、Pydantic、生成 TypeScript 和负例测试；S002 追加数据库表与 CAS；S003 只添加 GET/select/legacy rebuild hand-off API，不把 legacy ModuleGraph 伪装为可编辑资产。

## 后果

- S002 负责 SQLite 持久化与 revision compare-and-swap；
- S003 负责读/选择/显式 legacy conversion API；
- S004–S008 已使 desktop client/reducer、Agent 核心路径、不可变回退/前进和 legacy 只读重建接入 Snapshot；当前桌面 r3 Agent-first smoke 已验证参考 GLB v1 到可编辑资产 v2–v5 的版本链与重启恢复；
- Agent 工作台的 Snapshot 唯一状态已完成核心路径；文档仍必须把“整个兼容工作台生产级一致性”标为部分实现，直到广泛并发、原生安装恢复和 legacy UI 退出完成。

## 被否决方案

- 在旧/新 hook 间增加更多 if/else：无法提供版本、选择和导出的一致性保证；
- 用 localStorage 保存活动版本：无法处理重启、并发或跨设备数据；
- 把 legacy Version ID 改名为 Agent asset Version ID：会破坏历史数据和 provenance。
