# ForgeCAD 硬切重置与迁移清单

> 2026-08-25 商业质量计划不恢复旧 Provider、工作台、Blender Runtime、任意脚本或第二状态写者。未来新增的 High/Low/UV/Bake/Material/FPS executors 必须走 additive 原子任务和独立 Gate；本轮文档同步不授权删除、迁移或清理任何脏工作树文件。

版本：2026-08-13
状态：历史硬切已完成；后续 MVP 执行以 `MVP_DELIVERY_PLAN.md` 为准；ADR-0026 后续要求 active/archive 模块边界清晰，废弃材料先隔离再删除
决策依据：[ADR-0025](ADR/0025-codex-only-mcp-3d-runtime.md)、[ADR-0026](ADR/0026-agentic-design-runtime.md)

本文是 Luna 执行“先拆断旧工作台，再建设 Codex-only MCP Runtime”的唯一删除清单。没有列入“保留/迁移”的旧代码，不能因为测试依赖而恢复成产品路径。

## 0. 破坏性操作前置门（已完成，证据在 MCP001 handoff）

当前工作树在 reset 前存在大量未提交修改，删除目标与这些修改高度重叠。已完成：

1. base `b9693cd`、分支和 `git status` 已记录；
2. tracked diff、untracked archive、文件清单和 Git bundle 已保存；
3. `WushenForgeLibrary`（含 SQLite/CAS）已归档并生成 SHA-256；
4. 用户已授权按方案执行，reset 分支为 `codex/forgecad-mcp-reset`；
5. 临时恢复目录已验证 patch、archive、`library.db` 和 CAS 目录可读取；
6. 删除只在 reset 分支执行。

备份目录不得加入仓库。后续任何破坏性变更仍需新的恢复门。

## 0.1 2026-08-13 隔离补充规则

本文件的历史删除清单继续有效，但后续不再把“删除”作为第一动作。任何废弃文档、代码或模块先按 `DEPRECATED_ISOLATION_PLAN.md` 分类：

1. active 能力：保留在当前源树并有 Schema/tool/runtime/evidence；
2. target design：只在 ADR/plan 文档中描述；
3. superseded：移动到 `docs/evidence/archive/**` 或 `packages/forgecad-skills/archive/**`；
4. rejected：只保留 adoption/decision reason，不进入 active tree；
5. private backup：含用户数据、原图、绝对路径或旧运行状态时，保留在 Git 外。

本轮已将 superseded `reference-to-typed-plan@0.1.0` 移出 active Skill 根目录，放入 `packages/forgecad-skills/archive/superseded/reference-to-typed-plan/0.1.0`。后续新增废弃项必须先写隔离清单和恢复门，再移动或删除；当前脏工作树不得直接删除不明来源文件。

## 1. 文档清单

### 1.1 `FGC-MCP001` 直接删除

| 文档 | 原因 | 替代文档 |
|---|---|---|
| `docs/ADR/0023-deepseek-qwen-only-ai-provider-policy.md` | 内置 DeepSeek/千问决策失效 | ADR-0025 |
| `docs/ADR/0024-api-first-open-world-3d-coding-agent.md` | Provider Registry 和产品内 Agent 失效 | ADR-0025 |
| `docs/U004_STAGE1_HIGH_QUALITY_WORKBENCH_PLAN.md` | 旧工作台与 Provider 实施总图失效 | 本文、执行计划 |
| `docs/AGENT_PROVIDER_EVALUATION.md` | 不再评测内置模型 Provider | 能力门矩阵 |
| `docs/AGENT_CURRENT_ISSUES_AUDIT.md` | 旧 Agent 问题账本失效 | 文档状态、handoff |
| `docs/IMPLEMENTATION_PLAN.md` | 旧路线重复且冲突 | 执行计划 |
| `docs/DOMAIN_PACKS.md` | 旧 Domain Pack 路由失效 | Skill 标准 |
| `docs/MECHANICAL_DESIGN_OPERATIONS.md` | 旧机械语言失效 | 编译器管线 |
| `docs/MODULE_ASSET_GUIDE.md` | ModuleGraph 失效 | 资产创作 |
| `docs/MODULE_NAMING_STANDARD.md` | Module 命名真值失效 | Schema/Skill 标准 |
| `docs/COMPATIBILITY_MIGRATION.md` | 旧兼容入口删除 | 本文；旧库离线导出器 |
| `docs/AGENT_GITHUB_REFERENCE_ARCHITECTURE.md` | 旧 Agent 参考设计 | 外部项目采用清单 |
| `docs/AGENT_PLUGINS_SKILLS_DESIGN.md` | 旧 Plugin/Skill 设计 | Skill Package 标准 |
| `docs/EXTERNAL_REFERENCE_AND_PRODUCT_DIFFERENTIATION.md` | 与采用清单重复 | 外部项目采用清单 |
| `docs/API.md` | 旧 HTTP/JSON-RPC API | MCP Runtime 合同 |
| `docs/FRONTEND.md` | 旧工作台 UI | Viewer 合同 |
| `docs/evidence/U002_UNIVERSAL_AUTHOR_GATE.md` | 旧产品链 evidence | 新任务 evidence |
| `docs/evidence/U004_W4_INTEGRATION_EVIDENCE_MANIFEST.md` | 旧 U004 evidence | 新任务 evidence |
| `docs/evidence/f026/F026_VISUAL_SPEC.md` 与图片 | 旧 Codex 式聊天工作台 | 新 Viewer 基线 |
| `docs/examples/module-pack/**` | 旧 Module Pack | `packages/forgecad-skills` fixtures |

这些文件在文档重置提交中可暂时保留为“待删除 tombstone”，以承接当前脏工作区；`FGC-MCP001` 必须从当前树删除。历史只由 Git/重置归档保存，当前文档不得继续链接它们。

### 1.2 从头重写

`AGENTS.md`、`README.md`、`DOCUMENTATION_MAP.md`、`DOCUMENTATION_STATUS.md`、`PRODUCT_DEFINITION.md`、`DESIGN.md`、`AUTHORITATIVE_STATE.md`、`CODEX_EXECUTION_PLAN.md`、`CODEX_TASK_INDEX.md`、`CODEX_HANDOFF.md`、`LUNA_GOAL_EXECUTION_GUIDE.md`、`CODEX_DEFINITION_OF_DONE.md`、`USER_GUIDE.md`、`QUICKSTART.md`、`OPERATIONS.md`、`DEVELOPMENT.md`、`SCHEMAS.md`、`DATABASE.md`、`ASSET_AUTHORING.md`、`MATERIAL_SYSTEM.md`、`TEST_STRATEGY.md`、`PACKAGING.md`、`PRODUCTION_RELEASE_CHECKLIST.md`、`RELEASE_MAINTENANCE.md`、`DISASTER_RECOVERY.md`、`THIRD_PARTY_LICENSES.md`、`ADR/README.md`、`evidence/README.md`、`evidence/CAPABILITY_GATE_MATRIX.md`。

### 1.3 新增

- `ADR/0025-codex-only-mcp-3d-runtime.md`
- `RESET_MIGRATION_PLAN.md`
- `MCP_RUNTIME_CONTRACT.md`
- `CODEX_INTEGRATION.md`
- `COMPILER_PIPELINE.md`
- `WORKBENCH_VIEWER.md`
- `SKILL_PACKAGE_STANDARD.md`
- `EXTERNAL_PROJECT_ADOPTION.md`
- `ADR/0026-agentic-design-runtime.md`
- `FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md`
- `ARCHITECTURE_MODULE_BOUNDARY.md`
- `DEPRECATED_ISOLATION_PLAN.md`

## 2. 代码直接删除清单

### 2.1 旧桌面工作台

整个删除，无文件例外：

```text
apps/desktop/src/features/cad-workbench/**
```

它包含内置聊天、图片上传、Provider 设置、旧状态机、ModuleGraph renderer、PBR capture、drawer、smoke 和 CSS。新 Viewer 必须从 `apps/desktop/src/features/runtime-viewer/**` 从零建设，不能将 `WorkbenchShell` 或 `CadWorkbenchPanel` 换名复用。

删除旧前端桥接：

```text
apps/desktop/src/shared/api/appServerProtocol.ts
apps/desktop/src/shared/api/appServerTransport.ts
apps/desktop/src/shared/api/appServerTransport.smoke.ts
apps/desktop/src/shared/api/forgeApi.ts
apps/desktop/src/shared/api/forgeApi.*
apps/desktop/src/shared/api/packagedK001Probe.ts
apps/desktop/src/shared/api/packagedK002Probe.ts
apps/desktop/src/shared/api/packagedArmWebviewQa.ts
apps/desktop/src/shared/api/packagedC111BWebglQa*
apps/desktop/src/shared/generated/api-types.ts
apps/desktop/src/shared/tauri/agentSupervisor.ts
apps/desktop/src/shared/tauri/visionEvidence.ts
apps/desktop/src/app/providers/RuntimeProvider.tsx
```

完全重写 `apps/desktop/src/app/App.tsx`、`apps/desktop/src/main.tsx`。

### 2.2 Tauri 内置模型、Provider 和旧桥接

直接删除：

```text
apps/desktop/src-tauri/src/api_first_*.rs
apps/desktop/src-tauri/src/deepseek_*.rs
apps/desktop/src-tauri/src/provider_credentials.rs
apps/desktop/src-tauri/src/vision_evidence_adapter.rs
apps/desktop/src-tauri/src/research_gateway_adapter.rs
apps/desktop/src-tauri/src/coding_workspace.rs
apps/desktop/src-tauri/src/local_universal_provider.rs
apps/desktop/src-tauri/src/mvp_arm_*.rs
apps/desktop/src-tauri/src/c110g_packaged_probe.rs
apps/desktop/src-tauri/src/k003_packaged_probe.rs
apps/desktop/src-tauri/src/local_high_quality_visual_acceptance_probe.rs
apps/desktop/src-tauri/src/asset_render_compat.rs
apps/desktop/src-tauri/src/app_server_bridge.rs
apps/desktop/src-tauri/src/rust_core_runtime.rs
apps/desktop/src-tauri/src/rust_core_runtime/**
apps/desktop/src-tauri/src/rust_product_catalog.rs
apps/desktop/src-tauri/binaries/wushen-agent-*
```

完全重写 `apps/desktop/src-tauri/src/main.rs`。修改 `Cargo.toml`，删除旧 App Server/Protocol 和 Provider-only 网络/密钥依赖；修改 `tauri.conf.json`，移除 `wushen-agent`、端口 8000、旧 CORS/custom-resource 假设，改为新 Runtime/Worker 打包。

### 2.3 旧 Rust App Server 与协议

整 crate 删除：

```text
apps/desktop/src-tauri/crates/forgecad-app-server/**
apps/desktop/src-tauri/crates/forgecad-app-server-protocol/**
```

只迁移确定性算法和思想：取消、canonical hash、VisualProgram V2 lowering、受限 native executor、Schema 校验、权限、幂等和 readback。禁止迁移 `ProviderClient`、ActionLoop、Thread/Turn/Item、AgentProviderSnapshot、token/cost、coding/search/vision Provider、旧 JSON-RPC/HTTP/SSE、research gateway 和 Provider runner。

### 2.4 旧 Python Agent

确定性几何能力迁移到 `apps/geometry-worker/**` 后，删除整个：

```text
apps/agent/**
```

可迁移：受限 Geometry Executor、ShapeProgram、通用 profile/CSG/surface/PBR/UV/GLB/mesh-quality 算法。禁止迁移 FastAPI、uvicorn、端口 8000、Agent kernel/action loop/conversation/provider、机械 planner/domain pack、旧 SQLite writer 和旧生成 Schema Registry。新 Worker 只接受 Runtime 启动的受限 stdin/stdout 内部协议，不监听网络端口。

### 2.5 旧 Core 文件

直接删除且不进入新 Runtime：

```text
api_first_provider.rs
arm_design_intent.rs
arm_geometry_family.rs
c111_structural_detail.rs
c111_visual_fixture.rs
e005_formal_batch.rs
e005_provider_budget.rs
e005_visual_review_checkpoint.rs
forge_visual_author_source_v1.rs
forge_visual_authoring_intent.rs
forge_visual_program.rs
generation_gate_profile.rs
legacy_conversion.rs
neural_visual_generation.rs
single_generation.rs
visual_program_authoring_session_v2.rs
visual_reference_budget.rs
```

审计、迁移到新模块、复验后再删除原路径：

```text
canonical.rs error.rs filesystem_permissions.rs object_store.rs ownership.rs
artifact_readback.rs external_glb.rs shape_program.rs constraint_field_program.rs
forge_visual_program_v2.rs expanded_visual_dag_v2.rs high_level_visual_geometry_v2.rs
visual_geometry_patch_v2.rs surface_layers.rs component_recipes.rs
reference_evidence.rs reference_camera_fit.rs reference_camera_uv_bake.rs
reference_constraint_bundle.rs reference_constraint_fit.rs reference_multiview_fit.rs
projection_camera_binding.rs reference_appearance_binding.rs
geometry_invariant_binding.rs semantic_proportions.rs visual_convergence.rs
visual_quality_receipt.rs candidate_pbr_capture.rs
game_asset_profile.rs game_asset_lod.rs game_asset_delivery.rs
```

`models.rs`、`repository.rs`、`skills.rs`、`multimodal_design.rs`、`universal_authoring.rs`、`universal_asset_source.rs`、`assembly_delta.rs` 必须按新合同重写，不能原样复制。

### 2.6 旧 Schema、fixture 和数据库

删除：

```text
packages/concept-spec/**
packages/weapon-spec/**
packages/agent-skills/**
assets/module-packs/weapon-concept-v1-reference/**
migrations/0001_*.sql ... migrations/0054_*.sql   # 仅从新 Runtime 构建图删除；归档保留
```

新建 `packages/forgecad-contracts/**` 和独立 `migrations-runtime-v1/0001_runtime.sql`。旧库只读；新 Runtime 不打开旧数据库。需要导入时使用单独、显式、可复验的离线导出/导入工具。

删除旧 Schema 类别：Provider、3D Provider Job、external-generated-asset、coding/OCR/research、DeepSeek/Qwen、arm/C111/E005/mechanical/weapon/module graph、legacy concept、旧 Agent conversation 和 Provider budget。

### 2.7 脚本、测试和 CI

删除根 `package.json` 中 `deepseek-*`、`api-first-*`、`provider-*`、`k001/k002/k003`、`u004/e005`、`c110/c111`、旧 workbench、legacy concept/weapon、FastAPI/8000 和 Blender weapon pack 任务。

旧 `output/release/**` 只作重置归档，不得作为新架构发布 evidence；MCP001 清理当前构建输出后由新打包任务重新生成。

删除 Provider 与 U004 脚本：

```text
script/configure_deepseek_test_key.sh
scripts/run_deepseek_*
scripts/smoke_deepseek_*
scripts/check_ai_provider_policy.py
scripts/check_r4_provider_readiness.py
scripts/smoke_r4_provider_readiness.py
scripts/evaluate_r4_planner_truth_set.py
scripts/run_agent_provider_evaluation.py
scripts/smoke_api_first_contracts.py
scripts/adapt_u004_runtime_receipt.py
scripts/validate_u004_timed_acceptance.py
```

删除旧武器 Blender 脚本及旧测试工具；保留并重写 toolchain、安全、secrets、integrity、license/SBOM 和新 Runtime smoke 工具。Library 备份只在 reset 归档中完成，不把旧数据工具重新接入产品。重写全部 ForgeCAD CI，只保留 contracts、core/store、worker、MCP、Codex E2E、Viewer、质量、打包、安全和许可证 Gate。

## 3. 目标模块模型

```text
apps/desktop/src-tauri/crates/
  forgecad-contracts/       generated Rust contract types
  forgecad-core/
    design/                 SubjectProfile, RepresentationPlan, AssemblyGraph
    geometry/               typed programs, source maps, readback
    appearance/             UV, material, texture, bake
    quality/                structural + visual evidence compiler
    versioning/             candidate, change, immutable version, restore
    skills/                 bundle validation and recipe expansion
  forgecad-store/           new SQLite V1 + CAS
  forgecad-runtime/         single writer, jobs, approvals, orchestration
  forgecad-mcp/             Codex stdio adapter
  forgecad-worker-protocol/ bounded internal worker messages
apps/
  desktop/                  read-oriented Runtime Viewer
  geometry-worker/          restricted typed geometry compiler
  render-worker/            deterministic headless evidence
  blender-worker/           post-MVP optional fixed recipes only
packages/
  forgecad-contracts/       JSON Schema source of truth
  forgecad-skills/          first-party declarative bundles
```

依赖只允许向下：Viewer/MCP → Runtime → Core/Store → Contracts；Worker → Contracts/Core algorithm subset。Store、Worker、Viewer 和 MCP 均不能绕过 Runtime 写产品状态。

## 4. 新合同首批清单

`RuntimeCapabilities`、`RuntimeToolManifest`、`RuntimeJob`、`RuntimeJobEvent`、`Project`、`ActiveDesignSnapshot`、`DesignAssetVersion`、`Candidate`、`SemanticChangeSet`、`ApprovalReceipt`、`ReferenceEvidence`、`SubjectProfile`、`RepresentationPlan`、`GeometryProgram`、`VisualProgram`、`AppearanceProgram`、`MaterialGraph`、`TextureSet`、`UvLayout`、`RenderRecipe`、`RenderSet`、`QualityReport`、`ExplodedViewPlan`、`SkillBundleManifest`、`SkillEvalReport`、`ArtifactReadback`、`ExportManifest`。

每个 Schema 必须包含 `schema_version`、稳定 ID、项目 scope、canonical hash、创建者、父 lineage、时间和尺寸/预算；永久写合同还必须包含 `base_version_id`、`idempotency_key` 和审批绑定。

## 5. 必须升级的模块

| 模块 | 当前缺口 | 目标升级 |
|---|---|---|
| Geometry Compiler | 旧机械/visual program 混杂 | 通用曲线、截面、曲面、布尔、局部形变、稳定 source map |
| Appearance Compiler | 固定材质与工作台 capture | `MaterialGraph + TextureSet + UvLayout + BakeRecipe` |
| UV | 无正式真值和门 | 岛屿、padding、重叠、stretch、texel density、切线 |
| 材质/纹理 | 固定少量程序化表 | Principled PBR 语义、资产 provenance、烘焙和交付压缩 |
| Render Evidence | 依赖 WebView 打开 | 可重复 headless 固定视图和 AOV；Viewer 仅交互显示 |
| Quality Compiler | 结构门与视觉门分散 | 几何、轮廓比例、UV/PBR、纹理、细节、参考比较、Codex 评审分层 |
| Versioning | 两套版本/legacy 投影 | 不可变版本、typed delta、局部恢复、stale-base 冲突 |
| Exploded View | 当前不可用 | stable Part↔primitive、ExplodedViewPlan、碰撞/可读性验证 |
| Skill Registry | 旧 manifest + Operator | MVP canonical hash/Bundle/Benchmark/SBOM/provenance；分发签名/撤销后置 |
| Packaging | 旧 `wushen-agent` sidecar | Runtime、MCP、workers、Viewer 同版本签名打包 |

## 6. 硬切执行顺序

1. `FGC-MCP000`：权威文档重置；
2. `FGC-MCP001`：可恢复快照后，一次删除旧 UI/Provider/App Server/Agent/合同，并放入最小可编译 Viewer + Runtime skeleton；
3. `FGC-MCP002`：新合同、数据库、CAS、单写者；
4. `FGC-MCP003`：Codex-only MCP stdio 只读能力；
5. `FGC-MCP004`：单用户候选/审批/版本/restore/diagnostic-export 事务基座（done）；
6. `FGC-MCP005`：真实 Codex PNG/JPEG → CAS → ReferenceEvidence；
7. `FGC-MCP006`：typed design/geometry/appearance 合同和 MVP first-party Skills；
8. `FGC-MCP007`：硬表面机器人几何、语义 Part、稳定 ID、GLB readback；
9. `FGC-MCP008`：UV、切线、PBR、Viewer 真实 GLB、固定渲染；
10. `FGC-MCP009`：参考比较、局部修改、拒绝/批准、版本/restore、GLB export，关闭 MVP；
11. `FGC-MCP010A`：权威重排、用户级开发 App 激活和真实 Codex capability Gate；
12. `FGC-MCP010B`：V2 几何合同、真实 GLB/拓扑回读；
13. `FGC-MCP010C`：固定 perspective renderer、九 AOV、参考比较和 typed review；
14. `FGC-MCP010D`：受限高细节 Operator 和 first-party Skill 0.2；
15. `FGC-MCP010E`：first-party 离线硬表面 AssetPack、UV/PBR/纹理；
16. `FGC-MCP010F`：Viewer compare/selection/explosion、真实机器人闭环和人工门；
17. `FGC-MCP011`：Job、事件、取消、崩溃恢复、并发、GC 和性能；
18. `FGC-MCP012`：通用第三方 Skill/AssetPack 分发治理和签名撤销；
19. `FGC-MCP013`：Developer ID/notarized 打包、Codex packaged E2E、升级回滚、跨类别真人质量门。

MCP010A–F 的现行细节由 `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md` 取代原来“010 只做 Viewer”的窄描述；这不改变本文件对 MCP001–009 硬切历史的记录。

### MCP002 已交付边界

当前工作树已实现首批 13 个 Runtime Schema/Rust records、canonical JSON/hash、独立 V1 migration、旧库拒绝、SQLite WAL/foreign-key/busy-timeout、OS writer 文件锁、崩溃后锁释放、CAS 原子写入和备份恢复、authenticated Unix IPC，以及 MCP 的 Runtime IPC backend。MCP002 仍不开放几何、Appearance、Render、参考导入、Skill 执行或永久确认；这些能力必须在依赖任务完成后逐项启用。

## 7. 硬切退出 Gate

- 产品代码中 `DeepSeek|Qwen|DashScope|ProviderRegistry|model API key` 为零；
- 无端口 8000、FastAPI 产品服务和内置模型网络调用；
- Desktop 关闭时 Runtime 仍能 compile/render/evaluate；
- 同一项目只有一个数据库写者；
- MCP 重复请求幂等，stale base、取消、崩溃和重启均 fail closed；
- 未通过结构硬门的候选不能确认；
- 未经 Codex 中用户批准不能创建永久版本；
- 真实 Codex 上传参考 → MCP → 候选 → Viewer → 局部修改 → 回退 → 爆炸图 → 导出闭环有 receipt、GLB、固定视图和重启证据。
