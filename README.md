# ForgeCAD Runtime

ForgeCAD 正在从“内置模型与聊天的 3D Agent 工作台”硬切为“Codex 调用的本地 3D Runtime”。Codex 是大脑；ForgeCAD 提供类型化几何、UV/PBR、纹理材质、受限 Skills、渲染证据、质量编译、不可变版本、局部修改、爆炸图、回退和导出。

## 当前状态

2026-08-13：**MCP001–MCP009 的单用户 MVP host golden path 已完成，MCP010A Desktop 激活已完成，MCP010F 仍在推进**。当前源码口径为 100 Schema、35 read + 21 opt-in write = 56 tools；C/D/E/F 的固定 renderer、九 AOV、hard-surface Operator、AssetPack/PBR、Viewer compare/contour-first source surface 和 packaged read-model/core-control smoke 均只按其结构范围记录。Agentic observe/plan/critic/evidence projection 已通过隔离 source/transport probe；真实 Runtime 的嵌套只读 projection producer/consumer conformance 也已通过独立回执；durable `DesignSession`/`DesignCheckpoint`/`RepairIntent` 的受批准 prepare、Runtime SQLite/CAS 持久化、readback、恢复意图和 Viewer lookup 也已通过隔离重启 receipt，但 durable/reference/DesignSpec 完整 producer、单动作 orchestrator 和 Repair 应用仍未完成。真实机器人仍是 `QUALITY_TARGET_NOT_MET`，attempt35 只是 `provisional retained observation`，fit/compare camera 绑定不完整；像素级 likeness、真人视觉门、同 observation packaged Viewer、PBR likeness、export/restart hash、360 和 packaged release 仍分别标记 `NOT_RUN/BLOCKED`。

- 新方向已由 [ADR-0025](docs/ADR/0025-codex-only-mcp-3d-runtime.md) 接受；
- 后续高质量重规划已由 [ADR-0026](docs/ADR/0026-agentic-design-runtime.md) 记录：ForgeCAD 目标升级为 Agentic Design Runtime，让 Codex 通过 SemanticSceneGraph、ReferenceCanvas、DesignSpec、stage gates、Visual Evidence 和 Critic/Repair loop “看得见”并逐步设计；当前已落地第一阶段只读 projection，durable orchestrator/checkpoint/repair 仍是目标，不计入完整 Agentic Runtime 能力；
- 架构模块边界见 [ARCHITECTURE_MODULE_BOUNDARY.md](docs/ARCHITECTURE_MODULE_BOUNDARY.md)，废弃文档/代码/模块隔离规则见 [DEPRECATED_ISOLATION_PLAN.md](docs/DEPRECATED_ISOLATION_PLAN.md)；
- 旧 Provider、聊天工作台、App Server、Python Agent、旧合同和旧脚本已从当前树删除；
- superseded `reference-to-typed-plan@0.1.0`、`hard-surface-detail@0.1.0`、`uv-pbr@0.1.0` Skill provenance 已移到 `packages/forgecad-skills/archive/superseded/`，不属于 active registry 或 Runtime build archive；
- 新 first-party `ponytail-preflight@0.1.0` 强制 Codex 在每个 MCP 设计会话先检查必要性、既有受限能力与最小 typed action；它是 MIT workflow reference 的自有重写，不安装上游 Node package、hook 或 MCP server；
- 新 `forgecad-runtime`、`forgecad-mcp`、Runtime Viewer、worker protocol、Runtime V1 migration、首批 contracts、CAS、单写者和 authenticated local IPC 已通过本地 focused Gate；
- 当前版本可以进行开发构建上的本地 3D 功能评估，但不能宣称“通用高质量 3D”或已完成真实 Codex 视觉验收；
- [FGC-MCP004](docs/CODEX_TASK_INDEX.md) 已按 MVP 基座范围收口，且 `FGC-MCP005–009` 已完成各自功能核心：Runtime/authenticated IPC 候选、审批、restore-as-new-version、path-free diagnostic export、`forgecad-mcp` 内置轻量 supervisor、真实 Codex CLI diagnostic/reference write、PNG/JPEG ReferenceEvidence/CAS、有界 typed 多 Part mesh/GLB、bounded UV/tangent/PBR、fixed render、limited quality、stable-Part change 和 CAS `mvp-glb` receipt 已有 evidence；真实 Codex CLI 十二调用 host golden path 也已 PASS，MCP010A 已通过第二次 Desktop 激活 Gate。像素级参考相似度、人评、完整 Desktop 3D write 和 filesystem/package export 仍未验收；
- `FGC-MCP005–009 done` 的边界、命令和未运行项见 [MVP 交付计划](docs/MVP_DELIVERY_PLAN.md) 与 [证据清单](docs/evidence/)。`npm run mcp008:test` 和 `npm run mcp009:test` 分别覆盖 Appearance/Render/Viewer 与 Quality/Change/Version/Export functional core；签名公证和 packaged release 在 MCP013。真实 host receipt 见 `docs/evidence/mcp007/` 和 `docs/evidence/mcp009/`。

reset 前的未提交成果已归档并验证可恢复；归档不属于仓库，也不改变旧产品已经从当前树退役的决定。

## 目标体验

```text
用户在 Codex 对话并上传参考
        ↓
Codex 通过本地 MCP 调用 ForgeCAD
        ↓
ForgeCAD 编译 bounded 几何、UV/tangent、PBR MaterialZone 和固定视图
        ↓
Quality Compiler 生成结构与视觉证据
        ↓
用户在 ForgeCAD Viewer 查看，在 Codex 提出局部修改/批准
        ↓
Runtime 创建不可变版本，可回退、爆炸查看和导出
```

ForgeCAD Desktop 不包含聊天、图片上传、模型选择、Provider 设置或 API Key。P0 只支持 Codex Desktop 和 Codex CLI；Codex IDE / VS Code / Cursor / Windsurf 仅保留未来兼容基线，不是当前产品入口或发布 Gate。ForgeCAD 不内置任何模型 SDK 或模型调用。

未来目标体验在这个基础上增加：

```text
ReferenceCanvas
  → DesignSpec
  → SemanticSceneGraph / ModelUnderstandingBundle
  → Primary / Secondary / Tertiary stage gates
  → Visual Evidence Bundle
  → Critic / Local Repair
  → Human review / version / export
```

Primary form 未过时不进入细节，visible-view 未过时不解锁 PBR，`QUALITY_TARGET_NOT_MET` 不确认、不导出。

## 文档入口

- [文档地图](docs/DOCUMENTATION_MAP.md)
- [当前状态](docs/DOCUMENTATION_STATUS.md)
- [产品定义](docs/PRODUCT_DEFINITION.md)
- [架构设计](docs/DESIGN.md)
- [Agentic Design Runtime ADR](docs/ADR/0026-agentic-design-runtime.md)
- [Agentic Design Runtime 重规划](docs/FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md)
- [架构与模块边界](docs/ARCHITECTURE_MODULE_BOUNDARY.md)
- [废弃隔离计划](docs/DEPRECATED_ISOLATION_PLAN.md)
- [删除/迁移/升级完整清单](docs/RESET_MIGRATION_PLAN.md)
- [单用户 MVP 交付计划](docs/MVP_DELIVERY_PLAN.md)
- [MCP 合同](docs/MCP_RUNTIME_CONTRACT.md)
- [Codex 集成](docs/CODEX_INTEGRATION.md)
- [编译器与质量管线](docs/COMPILER_PIPELINE.md)
- [Viewer 合同](docs/WORKBENCH_VIEWER.md)
- [Skill Package 标准](docs/SKILL_PACKAGE_STANDARD.md)
- [外部项目采用清单](docs/EXTERNAL_PROJECT_ADOPTION.md)
- [工具/Skill/GitHub 候选目录](docs/MVP_TOOL_CATALOG.md)
- [Codex Ponytail 前置设计流程](docs/CODEX_PONYTAIL_PREFLIGHT_WORKFLOW.md)
- [Luna 执行指南](docs/LUNA_GOAL_EXECUTION_GUIDE.md)
- [原子任务索引](docs/CODEX_TASK_INDEX.md)

## 开发规则

开始实现前必须完整阅读 [AGENTS.md](AGENTS.md)。旧 U004/Provider/Module/Mechanical 文档已失去执行权威，不能用旧测试要求恢复已废弃架构。

本项目只生成合法的非功能性视觉资产，不提供现实武器制造图、制造尺寸、材料配方、加工流程、功能机构或性能建议，也不对交通、建筑、医疗或机械结果给出安全/认证结论。

更明确地说：虚构游戏美术资产只允许非制造说明；ForgeCAD 不输出可用于现实制造武器的精确图纸。
