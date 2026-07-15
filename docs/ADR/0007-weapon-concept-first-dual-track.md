# ADR-0007：Weapon Concept Pack 优先与双轨架构

- 状态：Superseded in product scope by ADR-0008
- 日期：2026-07-10
- 决策者：项目维护者
- 部分取代：ADR-0006 第 2–5 项涉及 P0 CAD/DFM 主链的决定

## 背景

维护者确认新产品不拒绝武器设计，并希望继续利用武器外形直观、组件丰富、细节密集和精密感强的特点作为第一阶段核心场景。同时，直接把 P0 定义成可工作的武器 CAD/DFM 系统，会同时引入 B-Rep、参数化特征、制造格式、结构验证和专业工程边界，显著延迟第一个可用闭环。

GPT Pro 方案建议把“武器题材入口”和“工程制造能力”拆开：先围绕未来概念、游戏资产、影视道具和非功能展示模型完成模块化 3D 工作流，再以独立能力包推进 CAD/DFM。

## 决策

1. ForgeCAD 是通用、本地优先的 AI 模块化 3D 设计平台。
2. 第一阶段内容包为 `Weapon Concept Pack`，不按武器类别拒绝需求，但正式承诺限定为未来概念、游戏资产、影视道具和非功能展示模型。
3. P0 权威模型为 `WeaponConceptSpec → ModuleGraph → GLB modules → combined GLB`。
4. P0 的 AI 修改只能生成结构化 `DesignChangeSet`；用户通过 ghost preview 确认后创建新版本。
5. P0 工作台阶段为“概念、组装、精修、检查、展示”；模型检查不称为 DFM，导出不称为制造就绪。
6. 首批使用 8–12 个高质量手工 GLB 和语义连接器跑通闭环，再扩展到 24–30 个。
7. `CAD / DFM Engineering Pack` 采用独立的 `DesignSpec → FeatureGraph → B-Rep → STEP/3MF → DFM` 权威链路，在后续里程碑实施。
8. Concept 项目不能自动被宣称为可工作、可制造、安全、合规或已认证的真实武器。

## 后果

- README、计划、设计、操作手册和工作台默认语义必须从“制造”调整为“概念交付”。
- R2–R6 的合同、数据库、API、模块系统、检查与导出围绕 Concept 主链实施。
- 原 CAD/DFM 设计作为 Engineering Pack 保留，不删除已形成的架构研究。
- Legacy CreativeWeaponGraph、神经 3D 和 Unity 仍不是新产品权威源。
- 演示样本继续使用模块化未来短武器，但不得包含真实工作机构或制造就绪声明。

## 被否决方案

- P0 拒绝全部武器题材：不符合维护者的明确定位，也失去最直观的垂直演示场景。
- P0 直接完成真实武器 CAD/DFM：范围过大，无法快速验证模块化 AI 工作流。
- 用生成式图片或神经粗模冒充可编辑 3D：不能稳定支持组件替换、连接检查和版本差异。
- 同时让 `ModuleGraph` 与 `FeatureGraph` 成为 P0 权威源：会产生双重几何真值和不可解释漂移。
