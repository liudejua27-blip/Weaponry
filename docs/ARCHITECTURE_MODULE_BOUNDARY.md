# ForgeCAD 架构与模块边界

版本：2026-08-13
状态：模块权责文档；描述当前已实现边界与 ADR-0026 目标模块。Agentic observe/plan projection、嵌套只读 projection conformance、受批准的 durable session/checkpoint/RepairIntent prepare/readback，以及 MCP010F 的窄范围 Primary Form 单动作 prepare/evaluate、bounded action-run/readback 已进入 Runtime/MCP/Viewer；通用 durable/reference/DesignSpec producer、完整单动作 orchestrator 和 Repair 应用仍未进入当前边界。

## 1. 总体边界

ForgeCAD 的边界固定为：

```text
External Agent Harness
  Codex Desktop / Codex CLI / future Pi-style harness
        |
        | MCP stdio
        v
forgecad-mcp
        |
        | authenticated local IPC
        v
forgecad-runtime
        |
        +-- SQLite V1 + CAS
        +-- Geometry / Appearance / Render Worker
        +-- Quality / Evidence / Versioning
        |
        v
Read-only Viewer
```

Codex/Agent 负责理解、规划、设计判断、选择工具和迭代。ForgeCAD 负责几何、约束、布尔/拓扑、单位、材质、渲染、版本、撤销、回读和质量证据。

## 2. 当前已实现模块

| 模块 | Owned state | 允许 | 禁止 |
|---|---|---|---|
| `forgecad-mcp` | 无数据库状态 | MCP initialize、tool/resource manifest、typed request validation、连接 Runtime | 打开 SQLite/CAS、执行模型、运行脚本、保存 Provider/API Key |
| `forgecad-runtime` | SQLite/CAS/Project/Candidate/Version/Job/Quality | 唯一写者、candidate/version/approval/export、Skill registry、QualityReport | 让 MCP/Viewer/Worker 写库、接受任意路径/URL/脚本 |
| Runtime Primary Form repair prepare | Runtime-owned staged candidate/CAS、RenderSet、Comparison、QualityReport | 接收一次 bounded typed intent，完成 Runtime-owned fit→GeometryProgram→strict readback→Render Worker→compare；只返回 staged candidate，failed quality 仍失败 | Codex 连续参数搜索、Viewer 重算质量、confirm/version/export、任意脚本 |
| Runtime Agentic action run | Runtime-owned SQLite/CAS `AgenticActionRunRecord` 与 action result projection | 接收一个已批准、单 Part、`primary-form` bounded action；复用 Runtime repair pipeline，返回可重放的 `DesignActionRun@1`，并通过 `design_action_run_get` 精确回读 | 多阶段自动推进、Repair 应用、candidate/version mutation、confirm/export、Viewer 本地质量推导、任意脚本 |
| Contracts | JSON Schema + canonical hash | 定义跨进程对象、版本、negative gates | 空 Schema 冒充能力、未实现 producer 就宣传 PASS |
| Geometry Worker | 临时 worker process | bounded typed Operator、GLB lowering、strict readback | 网络监听、任意 Python/JS/shell、下载资产、写 Runtime DB、渲染 AOV |
| Render Core / Render Worker | `apps/render-core` 无状态 renderer + `apps/render-worker` 一次性 worker process | 只接受 bounded self-contained GLB 与 typed camera，生成 fixed/perspective/batch AOV；Render Worker 不依赖 Geometry Worker crate | 编译 GeometryProgram、写 Runtime DB、网络/路径/脚本/模型调用 |
| Appearance/Render path | Worker/Runtime evidence | MaterialZone、UV/tangent、PBR、九 AOV、reference compare | 用 beauty/截图替代 QualityReport |
| Agentic projection | Runtime 按需派生的临时 projection；不持久化 | `scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get`；输出 observed/inferred/unknown、stage、gate、action 和 hash binding | 写 SQLite/CAS/candidate/version/checkpoint；把 projection 当 durable DesignSession 或视觉 PASS |
| Agentic durable prepare | Runtime-owned SQLite/CAS session/checkpoint/RepairIntent prepare/readback | `session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare`；要求 approval、project/candidate/reference/evidence binding，restore 只生成 CAS-only intent | 执行 orchestrator/Repair、直接改 candidate/version/history、把 prepare receipt 当视觉 PASS |
| Viewer | ephemeral UI state | 只读 GLB/AOV/compare/selection/explosion/heatmap | 创建版本、写 SQLite/CAS、保存产品状态到 localStorage |
| Skills/AssetPack | first-party manifests + receipts | 声明式 recipe、operator lock、validator、SBOM/provenance | 可执行插件、第三方仓库直接安装、模型权重 |
| Evidence | hash-only receipts | PASS/FAIL/BLOCKED/NOT_RUN 分层记录 | 用历史 receipt 证明当前 binary，或用结构 PASS 证明视觉 PASS |

## 3. ADR-0026 目标模块

以下模块仍是目标设计或后续 durable work；当前 Agentic projection 与 durable prepare/readback slice 已实现，不得把它扩展为完整 orchestrator：

| 目标模块 | 责任 | 落地要求 |
|---|---|---|
| Agent Harness Adapter | 线性 `Observe -> Plan -> Act -> Inspect -> Evaluate -> Checkpoint` 编排 | 不保存产品状态；所有动作仍走 MCP/Runtime |
| DesignSession | stage、checkpoint、失败门、下一步允许动作 | 当前已由 Runtime 受批准写入 SQLite/CAS 并可跨重启 readback；嵌套只读 projection conformance 已通过，但 durable/reference/DesignSpec 完整 producer、单动作 orchestrator 和 candidate/version mutation 仍未完成 |
| SemanticSceneGraph | Part tree、role、dimensions、symmetry、source map、editable parameters | 从 readback/RenderSet/Quality 派生，不由 Codex 本地猜 |
| ReferenceCanvas | reference coverage、views、observed/inferred/unknown | 绑定 CAS reference hash，缺失视图阻断 360 |
| DesignSpec | category、style、primary/secondary/tertiary goals、material language | 是设计合同，不是 prompt |
| Visual Evidence Bundle | 多视图 AOV、camera、selection、metrics、failed gate | 当前只读取既有 Viewer evidence；完整目标合同 conformance 和同 cohort packaged evidence 仍未完成 |
| Critic/Repair Loop | evidence-bound Part/MaterialZone issue 与 bounded repair | 不直接改几何，必须重新 compile/readback/render/compare |
| Parametric Design Kit | Housing/Panel/Vent/Joint/Sensor/Frame 等 intent | 展开为 typed bounded program，保留 source map |

## 4. 模块化目录原则

活动产品目录只放当前能力：

- `apps/desktop/src-tauri/crates/forgecad-runtime/**`
- `apps/desktop/src-tauri/crates/forgecad-mcp/**`
- `apps/geometry-worker/**`
- `apps/render-core/**`
- `apps/render-worker/**`
- `apps/desktop/src/features/runtime-viewer/**`
- `packages/forgecad-contracts/schemas/**`
- `packages/forgecad-skills/bundles/**`
- `packages/forgecad-assets/**`
- `docs/evidence/mcp*/**`

隔离目录只放历史或废弃材料：

- `docs/evidence/archive/**`
- `packages/forgecad-skills/archive/**`
- reset/private archive 路径，例如 `/tmp/forgecad-mcp001-20260807`

任何废弃代码、文档或模块不得继续留在活动目录根部；必须移动到 archive/quarantine，或删除前保留可恢复 receipt。当前脏工作树不得无证据删除用户数据或未提交修改。

## 5. 清晰架构验收

每个新增模块必须在文档里回答：

1. 谁是唯一写者；
2. 输入/输出 Schema 是什么；
3. 是否持久化；
4. 是否可重建；
5. 是否允许网络、脚本、路径、模型调用；
6. 对应 Gate 和 evidence 在哪里；
7. 与旧模块的隔离关系是什么。

如果回答不清楚，不允许进入 active capability。
