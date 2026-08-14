# ForgeCAD Codex / Luna 工作规则

本文件适用于整个仓库。2026-08-07 起，所有旧 Agent、Provider、U004 和工作台指令由 ADR-0025 取代。
2026-08-13 起，后续高质量路线由 ADR-0026 补充为 Agentic Design Runtime 目标架构；它不恢复产品内模型/Provider/聊天 Agent，也不改变 Runtime 唯一写者和无任意脚本边界。

## 1. 产品定义

ForgeCAD 是由 Codex 调用的本地、可验证、可回退 3D Runtime，不是内置大模型的独立 Agent 应用。

- P0 用户在 Codex Desktop 或 Codex CLI 中对话和上传授权参考；Codex IDE/VS Code/Cursor/Windsurf 只保留未来兼容能力，不是当前 ForgeCAD P0 入口；
- Codex 是外部大脑，负责理解、规划、视觉推理和工具编排；
- ForgeCAD 是身体，负责 typed 几何、UV、PBR、纹理、材质、渲染、质量、Skill、版本、局部修改、爆炸图和导出；
- ForgeCAD Desktop 只查看项目、候选、部件、参考、固定视图、质量、Job 和版本，不提供聊天、图片上传、模型选择、Provider 配置或 API Key；
- P0 只支持和验收 Codex，不内置 OpenAI、DeepSeek、千问或其他模型调用；
- MCP `stdio` 是 Codex 入口，Rust Runtime 是唯一产品状态写者。

“Codex-only”指支持范围，不是模型身份认证。不得用 `client_name == codex` 作为安全边界。图片附件必须通过真实 Codex 客户端证明字节进入 ForgeCAD CAS；没有证据时标为 unavailable。

当前是单用户 MVP host golden path 已收口、MCP010C/D/E source Gate 已通过、MCP010F Viewer source、packaged CLI read-model、原生窗口和打包核心控件 AX smoke 已通过的开发阶段，不是通用高质量产品。MCP002–MCP009 的 Runtime/MCP/Worker/Viewer/事务能力已有 focused evidence；真实 Codex CLI 已用用户授权图片完成 reference→geometry→appearance→quality→confirm→CAS GLB 十二调用主链路。MCP010C 当前源码已实现固定 512×512 perspective/z-buffer renderer、九 AOV、candidate-bound reference comparison、Codex/human review 与 MCP image block raw Gate；首次真实机器人 PNG 的 compare/review transport 已运行但 likeness threshold 未通过。MCP010D 的真实硬表面 Operator、MCP010E 的离线 AssetPack/UV/PBR/MikkTSpace source Gate 已通过；MCP010F 已补齐只读 Viewer 的 AOV/对比/部件筛选/材质区筛选/爆炸图 source surface，并由同 cohort Dev.app CLI 读取隔离 candidate 的 read model，原生窗口结构探针和 Computer Use 核心控件 smoke 也已通过。Agentic Phase 1 的 observe/plan/critic/evidence projection 仍是只读投影，真实 Runtime 的嵌套只读 projection producer/consumer conformance 已通过独立回执；本轮另已实现受批准的 Runtime-owned durable `DesignSession`/`DesignCheckpoint`/`RepairIntent` prepare、SQLite/CAS 持久化、MCP readback 与 Viewer durable lookup，隔离 Runtime/MCP 重启探针通过。该 durable slice 不包含 durable/reference/DesignSpec 完整 producer、单动作 orchestrator 执行、Repair 应用或用户持久候选变更。正式 VoiceOver 无障碍、第三方 xatlas/Validator、独立人视觉门、真实 likeness、导出重启 hash 和 360°仍是 `NOT_RUN/BLOCKED`，不能宣称“Codex 已生成通用高质量 3D”。

ADR-0026 的单动作写入 orchestrator、Repair 执行和 durable/reference/DesignSpec 完整 producer conformance 仍是目标设计。当前 `session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare` 已有 source/runtime/MCP/Viewer 和隔离重启 receipt，真实 Runtime 的嵌套只读 projection 另有 conformance receipt；这些能力只能分别写成 `nested projection conformance PASS` 与 `durable prepare/readback PASS`，不等于完整 Agentic Runtime、Repair 已执行或视觉 PASS。废弃文档、代码和模块按 `docs/DEPRECATED_ISOLATION_PLAN.md` 进入 archive/quarantine；不得在脏 worktree 直接删除未知文件。

## 2. 唯一权威阅读顺序

开始任何任务前完整阅读：

1. `docs/DOCUMENTATION_MAP.md`
2. `docs/DOCUMENTATION_STATUS.md`
3. `docs/CODEX_HANDOFF.md`
4. `docs/ADR/0025-codex-only-mcp-3d-runtime.md`
5. `docs/ADR/0026-agentic-design-runtime.md`
6. `docs/FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md`
7. `docs/ARCHITECTURE_MODULE_BOUNDARY.md`
8. `docs/DEPRECATED_ISOLATION_PLAN.md`
9. `docs/RESET_MIGRATION_PLAN.md`
10. `docs/CODEX_EXECUTION_PLAN.md`
11. `docs/CODEX_TASK_INDEX.md`
12. `docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`（MCP010A–F 执行时必读）
13. `docs/AUTHORITATIVE_STATE.md`
14. `docs/MVP_DELIVERY_PLAN.md`
15. `docs/MVP_TOOL_CATALOG.md`
16. `docs/EXTERNAL_PROJECT_ADOPTION.md`
17. `docs/LUNA_GITHUB_REPLICATION_PLAYBOOK.md`（GitHub 研究或选择性源文件复刻时必读）
18. `docs/CODEX_PONYTAIL_PREFLIGHT_WORKFLOW.md`（经 MCP 设计 3D 或修改 Skill/MCP 顺序时必读）
19. `docs/LUNA_GOAL_EXECUTION_GUIDE.md`（Luna/Goal 执行时必读）
20. 与任务直接相关的合同：MCP、Codex、Compiler、Viewer、Skill、Schema、测试或打包文档。

旧 ADR、U004 总图、Provider、Domain、Mechanical、Module 和 Compatibility 文档已从当前树删除，没有执行权威。不得从 Git 历史恢复旧产品路径来让测试通过。

## 3. 强制实施顺序

实施顺序固定为 `FGC-MCP000 → MCP001 → ... → MCP009 → MCP010A → ... → MCP010F → MCP011 → MCP012 → MCP013`，详见任务索引。同一时刻只领取一个原子任务。MVP functional core 主线 `MCP005 → MCP006 → MCP007 → MCP008 → MCP009` 已完成；`MCP010A` 已完成真实 Codex Desktop 激活 Gate；MCP010D/E source-focused Goal 已完成；MCP010F 当前为用户继续推进的唯一 `in_progress` 原子任务，范围限定为只读 Viewer source/构建和真实闭环证据整理。MCP010B 的 Darwin OS 总内存硬门仍 `NOT_RUN`，作为 deferred prerequisite 保留账本；MCP010C 的真实视觉门和 MCP010F 的 packaged/human/360 子门仍独立记录。

`FGC-MCP001`–`FGC-MCP009` 已完成当前功能核心定义；`MCP010A`–`MCP010F` 是首个硬表面参考质量产品化轨道，`MCP011`–`MCP013` 保留可靠性、分发和正式发布职责：

1. MCP004 已提供 candidate/Job/approval/confirm/reject/restore/diagnostic-export 事务、OS 文件锁、MCP 内置轻量 Runtime supervisor、真实 Codex CLI diagnostic write 和 Viewer read model；
2. distribution signing、notarization、Desktop packaged write 和通用多客户端治理移到 MCP013，开发期不得让它们阻塞参考导入与几何能力；
3. MCP005 已完成真实 PNG/JPEG 附件字节 → CAS → `ReferenceEvidence`，不得把它扩写成几何完成或引入任意脚本插件；
4. MCP006 已完成其历史 44 个 typed contracts、十个独立 first-party declarative Bundle、Registry 只读暴露、DAG/单位/finite/预算/hash/license/SBOM/provenance 负向 Gate；MCP010B/C/D/E/F 与 Agentic Design Runtime 当前 source contracts 使 manifest 为 100 个。Agentic 合同已通过正/负 fixture checker；真实 Runtime 的嵌套只读 projection producer/consumer conformance 已通过独立 receipt，durable session/checkpoint/RepairIntent prepare/readback 另有隔离 receipt，但不等于 durable/reference/DesignSpec 完整 producer conformance、写入 orchestrator 或视觉质量通过；已安装 Dev.app receipt 仍是历史 cohort，不能把它冒充为本轮 package/live Desktop 证据；
5. MCP007 已完成 product-owned bounded box/cylinder/sphere compiler、14 部件机器人 fixture、GLB lineage/readback、Runtime/MCP/Viewer authenticated IPC focused Gate；真实 Codex CLI 已用用户授权 PNG 完成 geometry/readback slice（14 parts/516 triangles/validator passed），MCP009 证据另含 appearance/quality/confirm/export 主链路；不得把有限主链路扩展成像素相似度或通用质量结论；
6. IDE/其他 Client 和 transport-specific official conformance 保持未来/非阻塞状态，不得伪造 PASS。
7. MCP008 的 bounded Appearance/Render/Viewer focused Gate 已通过；MCP009 的 Runtime golden-path/change/export focused Gate 已通过。MCP010 不能改写这些历史 receipt，只能新增 V2 合同、质量和真实视觉证据；真实 Codex、真人和 packaged gates 必须继续单独标记，不能用本地 fixture 代替。
8. MCP010A 只做权威重排、同 revision 开发构建/用户级激活和真实 Codex capability Gate；第二次完整重启已证明真实工具、Runtime Ready、能力 cohort 和临时项目读回，故 010A 可标记 `done`。MCP010B 当前结构 Gate 已通过，但 Darwin 512 MiB OS 总内存硬门仍 `NOT_RUN`，不把受限输入预算或 peak-RSS 后验拒绝写成预防式硬上限。MCP010C 的固定 renderer/九 AOV/reference compare/review source Gate 已通过，但真实机器人 likeness 仍为 `QUALITY_TARGET_NOT_MET`。MCP010D source Goal 已完成：当前源码默认有 35 个只读工具，显式 write opt-in 后为 35 read + 21 write = 56；Operator Catalog 现有 13 项、12 项 active，`boolean@1` 明确 unavailable；profile/loft/revolve/sweep/transform/mirror/array/panel/vent/joint/part-output 已由固定 Worker 实际执行，`hard-surface-detail@0.2.0` 仅在 bundle integrity/benchmark/provenance 验证后 active。D source Gate 与同 cohort packaged D raw structural probe 已通过；Manifold boolean、视觉和人评仍未运行。MCP010E source Goal 已通过当前 100 contracts、离线 `forgecad-hard-surface-robot@1.0.0`、512px bounded UV atlas、固定 `mikktspace@0.3.0`、嵌入式 PNG metallic-roughness/normal/AO/emissive/clearcoat 和九 AOV raw Gate；同 cohort packaged E 结构性用户参考→detail→AssetPack→compare 传输也已通过；xatlas、Khronos Validator、视觉/PBR likeness、export/restart hash 和 360°仍未运行。MCP010F 当前补齐只读 Viewer source、packaged CLI read-model、原生窗口和核心控件 AX smoke，并新增 Runtime 轮廓目标、扩展相机搜索、`SilhouetteRig@1` bounded fit、Runtime-owned Rig hash、SDF/Chamfer、单 Part contour proposal 和 candidate compare source Gate：AOV、参考分屏/叠加/闪烁、Part/MaterialZone 筛选、爆炸图和差异热图辅助均已通过 TypeScript/Vite/Tauri source Gate；Agentic observe/plan/critic/evidence projection 及 durable session/checkpoint/RepairIntent prepare/readback 也已通过隔离 source/transport/restart probe；正式 VoiceOver/无障碍、真实用户闭环、人评和 360°仍未运行。

不得在当前脏 `main` 上直接删除。不得跳过 MCP001 继续扩展旧工作台或修 Provider。

## 4. 不可违反的架构约束

- `forgecad-runtime` 是 SQLite/CAS/项目/候选/版本/Job/Skill 的唯一写者；
- `forgecad-mcp` 是无数据库的薄 `stdio` 适配器；
- Viewer 只有 read model 和临时 camera/selection/isolation/explosion 状态；
- Worker 只接受受限 typed 内部协议，不监听网络，不执行任意脚本；
- 同一候选的 Geometry/Appearance/Render/Quality/Export 共享 ID、hash 和 lineage；`change_prepare` 记录 stable Part intent，但不宣称通用 mesh delta；
- `ActiveDesignSnapshot` 是当前项目的单一状态投影；
- 所有永久修改先 prepare，后编译/回读/质量，再由用户在 Codex 批准，最后 confirm；
- 确认创建不可变子版本；restore 创建新版本，不改写历史；
- 大文件进入 CAS，事件/日志只保存引用；
- 新 Runtime 使用全新 V1 数据库，不自动打开旧 Library；旧数据只读保存并由一次性工具显式导出；
- 无任意 Python、JavaScript、shell、URL、文件路径、环境变量或 secret 进入几何真值；
- 不使用 Provider Registry，不读取或存储 Codex/OpenAI/DeepSeek/千问 API Key。

## 5. Skill 约束

正式分发 Skill 必须同时包含：

`知识 + typed Schema + Recipe DAG + 受限 Operator + Validator + 材质/资产 + Benchmark + LICENSE/NOTICE/SBOM + provenance + signature`

MVP first-party Bundle 只含声明式内容；可执行 Operator 必须是产品预注册实现。开发阶段可使用仓库 first-party trust root + canonical hash，但 Schema、Recipe、operator lock、Validator、Benchmark、LICENSE/NOTICE、SBOM 和 provenance 不能省略；分发级签名/撤销在 MCP012/013 完成。GitHub 仓库不能直接安装为 Skill。
每个 MCP 会话在使用 ForgeCAD 设计工具或读取其他 Skill 前，必须先 `skill_get` 读取 first-party `ponytail-preflight@0.1.0`；该前置步骤仅约束编排顺序，不是几何执行、质量通过或用户批准。上游 Ponytail 的 Node package、hook 和 MCP server 不得安装或执行。
Superseded Skill 不得留在 active Skill 根目录或 `bundles/**`；只允许保存在 `packages/forgecad-skills/archive/**`，不得进入 `registry.json` 或当前能力计数。

## 6. 高质量定义

“高质量”至少需要同一 candidate hash 的：

- 合同、预算、几何和严格 GLB readback；
- 语义 Part/MaterialZone/source map；
- 多视图轮廓与比例；
- UV/tangent、PBR 通道、纹理、材质和 provenance；
- 固定相机的 beauty/depth/normal/AO/part-ID/material-ID/wireframe/UV-stretch/silhouette；
- 参考比较、Codex typed visual review 和独立真人门；
- preview/export/restart 同一版本和 hash。

Skill 安装、材质包、单张截图、GLB 可打开、本地 smoke 或 Codex 自评不能替代这些证据。

## 7. Blender 与外部项目边界

可以学习 Blender 的 data-block、Modifier/Geometry Nodes、Principled PBR、UV/UDIM/Bake、AOV、OCIO、Asset Browser 和 Outliner；不能把 `.blend`、任意 Blender Python 或 Blender 内部状态变成产品真值。

外部项目按 Library、isolated Worker、Asset 或 Reference-only 分离采用。`MVP_DELIVERY_PLAN.md` 中 `approved-for-evaluation` 只授权隔离评估，不等于采用；写入 lockfile/安装包前必须固定 commit/tag、审许可证和例外、生成 SBOM、运行恶意输入/资源/确定性 Benchmark并保留退出方案。未经批准不得下载权重、执行安装脚本或整仓复制。用户已对 build123d、BlenderMCP、CadQuery、Manifold 和 MaterialX 授权 Luna 进行选择性源文件研究；该权限只能按 `docs/LUNA_GITHUB_REPLICATION_PLAYBOOK.md` 落到隔离缓存和 `research-authorized` receipt，不能变成直接依赖、active Skill 或 Runtime 代码。MCP010E 可由 Codex 在实施期把计划中点名的免费 CC0 文件一次性下载到本机 adoption cache，经逐资产 hash/license/SBOM/provenance 验证后编入 first-party 离线 AssetPack；Runtime、安装器和 Viewer 仍不得联网或调用素材 API。BlenderMCP、FreeCAD MCP 等任意脚本型插件不得接入 Runtime。

## 8. 安全范围

ForgeCAD 面向合法的非功能性 3D 视觉资产。未来虚构武器只限游戏美术、影视道具和展示；项目不生成现实可制造武器、制造图、制造尺寸、材料配方、加工流程、性能或操作建议。汽车、飞机、建筑、角色和机械结果不提供结构、安全、适航、医疗、动力学或认证结论。

参考图片和资产必须由用户有权使用；导出保留 license/provenance。日志、receipt、MCP 输出和包内不得泄露 secret、prompt、图片原始字节、本机用户名或绝对路径。

## 9. 任务规则

任务开始：

- 记录任务 ID，确认依赖和唯一 `in_progress`；
- 运行 `git status -sb`、`git diff --check`；
- 阅读任务代码入口、Schema 和 Gate；
- 记录基线结果，保护用户未提交修改；
- 若任务含删除，先满足重置恢复门。

任务结束：

- 只在退出条件全部满足时标为 `done`；
- 成功、失败、阻断、未运行分别记录；
- 更新任务索引、状态账本、能力矩阵和 handoff；
- 不用“基本完成”“应该可用”替代证据；
- 除非用户明确要求，不 commit、merge 或 push。

## 10. 基线 Gate

文档/合同变更至少运行：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
git diff --check
```

代码重置后，CI 只保留 contracts、core/store、geometry/render workers、MCP conformance、Codex E2E、Viewer、quality、packaging、安全和许可证 Gate。旧 Provider/U004/workbench Gate 必须删除，不得放宽新 Gate 来换绿色。

## 11. 完成定义

MVP 功能核心在 `FGC-MCP005`–`FGC-MCP009` 已完成；`FGC-MCP010A`–`FGC-MCP010F` 负责把它升级为“首个硬表面可见视图质量闭环已验收”。当前单张三分之四参考最多产生 `PARTIAL_VISIBLE_VIEW_PASS`；用户补充 front/back/left/right/rear-three-quarter 全身参考之前，`HQ_360_PASS` 必须为 `BLOCKED_REFERENCE_COVERAGE`。正式分发仍要求 MCP011 的可靠性、MCP012 的通用第三方生命周期，以及 MCP013 的 Developer ID/notarization、升级回滚、packaged E2E 和跨类别真人门。
