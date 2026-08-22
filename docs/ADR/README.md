# ForgeCAD 架构决策索引

数值口径：当前合并 source 为 **411 schemas / 28 operator catalog entries / 91 read + 69 write = 160 tools**；本文较小的数量仅作 historical prior slice 保留。

当前树保留 `ADR-0025-codex-only-mcp-3d-runtime.md`、`ADR-0026-agentic-design-runtime.md` 和 `ADR-0027-native-fps-weapon-production-executor.md`。旧 ADR 已从产品树删除，历史查询依赖 reset archive/Git history，不得作为实施合同或能力证据。

ADR-0025 已于 2026-08-09 增补单用户 MVP profile：OS 文件锁、薄 MCP、可选 Viewer、MCP005–009 functional core、工具/Skill 目录与 MCP010–013 发布分界。该增补收窄实施复杂度，没有改变 Codex-only、Runtime 唯一写者、typed Operator 和无任意脚本的核心决策。

ADR-0026 于 2026-08-13 增补 Agentic Design Runtime 目标架构：在 ADR-0025 的 Runtime/MCP/Viewer 边界上，新增 DesignSession、SemanticSceneGraph、ReferenceCanvas、阶段质量门、Visual Evidence Bundle 和 Critic/Repair loop 的目标设计。observe/plan read-only projection、durable session/checkpoint/RepairIntent prepare/readback 与窄范围 `repair_intent_run_prepare` source slice 已实现；Mechanical pose 单 tick/sequence 另为 candidate-bound read projection；单动作 orchestrator、Repair 应用和完整视觉闭环仍未完成。当前源码为 187 Schema、21/21 active operators、54 read + 35 opt-in write = 89 tools；旧 102 Schema/59 tools 与中间 144/156/158/160/162/164/168/175/177 Schema slices 仅保留为历史 cohort 快照，MCP010F 仍为 `QUALITY_TARGET_NOT_MET`。

ADR-0027 于 2026-08-23 将第一垂直目标收紧为原生 FPS 武器美术生产执行器：明确没有 Blender 软件、Blender Worker、Blender runtime 或 fallback；把现有 self-surface `CandidateSurfaceBake@1` 与真正 High/Low/Cage Bake 分开；定义 Hero UV、精细 ProductionStage@3、分阶段视觉门、真人艺术门和引擎门。它是目标架构，不把当前灰模或 source Gate 升级为商业质量。

后续相反架构变更必须新增编号，说明替代范围、数据迁移、兼容性、许可证和退出条件。
