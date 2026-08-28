# ForgeCAD 架构决策索引

> 2026-08-26 现行 source 口径：**527 schemas / 28 operators / 115 read + 87 write = 202 tools**；真实 D1 已有 stable-ID mesh edit/durable/worker/readback/six-view 纵切，但仍被 fresh FormArt owner evidence 阻断，商业 High→Low→UV→Bake→Material→FPS→Engine→Human 闭环尚未完成。

- `0027-native-fps-weapon-production-executor.md` 的商业质量解释和实施合同由 `../COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md` 统一展开；该计划不改变 ADR-0027 的 accepted architecture，也不把目标模块或第三方评估写成已实现。

数值口径：2026-08-26 当前合并 source 为 **515 schemas / 28 operator catalog entries / 111 read + 83 opt-in write = 194 tools**；本文较小的数量仅作 historical prior slice 保留。工具和合同数量只表示协议表面，不是商业视觉质量 KPI。

当前树保留 `ADR-0025-codex-only-mcp-3d-runtime.md`、`ADR-0026-agentic-design-runtime.md`、`ADR-0027-native-fps-weapon-production-executor.md` 和仅作非产品研究归档的 `ADR-0028-blender-headless-worker-evaluation.md`。旧 ADR 已从产品树删除，历史查询依赖 reset archive/Git history，不得作为实施合同或能力证据。

ADR-0025 已于 2026-08-09 增补单用户 MVP profile：OS 文件锁、薄 MCP、可选 Viewer、MCP005–009 functional core、工具/Skill 目录与 MCP010–013 发布分界。该增补收窄实施复杂度，没有改变 Codex-only、Runtime 唯一写者、typed Operator 和无任意脚本的核心决策。

ADR-0026 于 2026-08-13 增补 Agentic Design Runtime 目标架构：在 ADR-0025 的 Runtime/MCP/Viewer 边界上，新增 DesignSession、SemanticSceneGraph、ReferenceCanvas、阶段质量门、Visual Evidence Bundle 和 Critic/Repair loop 的目标设计。observe/plan read-only projection、durable session/checkpoint/RepairIntent prepare/readback 与窄范围 `repair_intent_run_prepare` source slice 已实现；Mechanical pose 单 tick/sequence 另为 candidate-bound read projection；单动作 orchestrator、Repair 应用和完整视觉闭环仍未完成。当前源码口径为 515 schemas、28 operator entries、111 read + 83 opt-in write = 194 tools；490-schema/184-tool Stage0 及更早数字仅保留为历史 cohort 快照，MCP010F 仍为 `QUALITY_TARGET_NOT_MET`。

ADR-0027 于 2026-08-23 将第一垂直目标收紧为原生 FPS 武器美术生产执行器：明确没有 Blender 软件、Blender Worker、Blender runtime 或 fallback；把现有 self-surface `CandidateSurfaceBake@1` 与真正 High/Low/Cage Bake 分开；定义 Hero UV、精细 ProductionStage@3、分阶段视觉门、真人艺术门和引擎门。它是目标架构，不把当前灰模或 source Gate 升级为商业质量。

ADR-0028 的固定 Blender headless Worker 产品评估入口已被 ForgeCAD-only 商业质量路线取代；文件只保留非产品威胁模型与许可证研究。它不授权执行、分发、lockfile/package/Runtime allowlist 变更，也不把 Blender、Python bundle 或 `.blend` 变成产品真值；产品 capability 固定 `UNAVAILABLE_FOR_PRODUCT`。

后续相反架构变更必须新增编号，说明替代范围、数据迁移、兼容性、许可证和退出条件。
