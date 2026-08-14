# FGC-MCP010 高质量硬表面参考闭环计划

版本：2026-08-13
状态：`FGC-MCP010A done`；`FGC-MCP010B blocked/deferred（Darwin OS memory hard cap NOT_RUN）`；`FGC-MCP010C source-focused PASS_WITH_UNRUN_VISUAL_GATES`；`FGC-MCP010D source-focused PASS_WITH_DEFERRED_BOOLEAN_AND_VISUAL_GATES（当前 packaged D 结构性探针 PASS，视觉门 NOT_RUN）`；`FGC-MCP010E source-focused PASS_WITH_DEFERRED_EXTERNAL_GATES（当前 packaged E 结构性探针 PASS，但视觉/人评/导出仍 NOT_RUN）`；唯一 `in_progress` 为 `FGC-MCP010F`（Viewer source、packaged CLI read-model、原生窗口与核心控件 smoke PASS；同一 provisional observation 的 packaged Viewer 绑定、正式 VoiceOver、人评和 360 仍 `NOT_RUN/BLOCKED`）。ADR-0026 已新增 Agentic Design Runtime 目标架构；它不改变当前 F 状态。

2026-08-14 Primary Form bilateral Part-ID projection：Runtime boundary proposal 现在把 Render Worker 的显式左右 Part-ID 合并到 Rig 的 `*-pair` semantic Part，再计算局部 envelope 的 width/height/offset proposal；这修复了 dominant pair 已命中但 pair proposal 仍为 authored baseline 的收敛断点。pair focused/full Runtime、MCP010C/F source 与 Render Worker boundary Gate 通过；没有新的授权参考字节或视觉 receipt，质量与 benchmark 真值保持不变。

2026-08-14 Primary Form 单 Part repair scope：Runtime 现在把主导 candidate-bound Part-ID boundary error 设为一次 bounded repair 的唯一 mutable scope，其他 Part 的 typed proposal 恢复 authored baseline；`DesignCriticReport@1` 的 Codex-facing repair operation 直接指向 `primary_form_repair_prepare`，避免把 `silhouette_fit_prepare` 重新暴露成连续参数搜索。Runtime/Agentic contract/MCP010C/F/Render Worker boundary Gate 通过；这仍只是收敛与编排修复，没有新的授权参考字节或视觉 receipt，现有质量与 benchmark 阻断不变。

2026-08-14 Primary Form Part-priority follow-up：在 Runtime 已产生 candidate-bound Part-ID boundary segments 后，bounded geometry probe 排序优先于主导 Part 的聚合 contour distance，再以 typed proposal delta 和稳定参数 ID 排序；没有 Part-ID evidence 时沿用旧 fallback。此模块修复只改善有限预算的误差覆盖，不是 likeness 门；本轮真实复验因授权参考原图字节不可用而 `BLOCKED_REFERENCE_BYTES_NOT_AVAILABLE`，当前质量与 benchmark 真值不变。
依赖：`FGC-MCP009 done（MVP host golden path）`

当前账本校正：源码合同为 102 个 JSON Schema，工具面为 35 个默认只读工具和 22 个显式 opt-in write 工具（共 57）。新增 `silhouette_part_error_get` 与 `primary_form_repair_prepare`；后者把一次 bounded fit→typed GeometryProgram→strict readback→Render Worker→compare 收口为 Runtime-owned staged prepare，不 confirm/version/export。Agentic projection 与 durable session/checkpoint/RepairIntent prepare/readback 另有独立 receipt；它们仍是结构/编排能力，不是 likeness 通过。

2026-08-14 Primary Form 首轮全控制覆盖修复：26-control detail Rig 的 CLI fit budget 由 32 提升为 64；Runtime 在 GeometryProgram 路径固定执行 `32 geometry + 16 initial-camera + 16 geometry-winner-camera-refit`，几何阶段先跑一次证据提案，再逐一覆盖 26 个控制，剩余几何预算才进入反向方向。该路径仍是 Runtime-owned bounded typed search，Codex 不接收或驱动连续参数轨迹。26-control focused/full Runtime、MCP010C/F source Gate 通过；没有新的真实机器人视觉 receipt，`QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 和 human/PBR/export-restart/360 未运行状态保持不变。

2026-08-14 Primary Form 肢段尺度/装配控制扩展：detail probe 在现有 `SilhouetteRig@1` typed `height` 与 `offset_y` 语义上，从 20 个控制扩展为 26 个，新增 upper-arm/forearm/thigh/shin height 与 elbow/knee vertical placement。Runtime 继续以 DAG-aware materialization 和有界确定性 schedule 承担参数试算，Codex 只提交一次 typed Rig；没有新的授权机器人视觉 receipt。focused/full Runtime、MCP010C 与干净 worktree 的 MCP010F source Gate 通过，当前机器真值仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 仍 `NOT_RUN/BLOCKED`。

Stage 0 机器真值唯一入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。attempt35 只是 provisional retained observation，不是已成立 benchmark：它是 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`；packaged Viewer 的 current-cohort read-model binding 已单独通过，但不等于 attempt35 的 packaged visual E2E。当前 r3 只证明 Runtime-owned single-action transport，仍不能越过 PBR likeness、独立真人、export/restart 或 360 门。

2026-08-14 Agentic visual evidence consolidation：target-bound `SilhouetteTarget@1` is now carried as nullable `target_sha256` in Runtime visual evidence and read-only Viewer/Agentic lineage. `DesignCriticReport@1` exposes a fixed-priority `primary_form_directive` and one Runtime-owned bounded RepairIntent for Primary Form failures；Codex receives one coherent action context and does not search continuous parameters. Target-bound Runtime round-trip, full Runtime/Store tests, contract checkers and `script/test_mcp010f.sh` passed. This is evidence/context infrastructure only：Viewer still consumes Runtime quality truth, Repair is not executed, and real visual quality remains `QUALITY_TARGET_NOT_MET` with `MISMATCH`/`BLOCKED_INCOMPLETE_BINDING`.

2026-08-14 Viewer evidence lineage follow-up：Runtime `visual_evidence` now validates the complete candidate artifact → RenderSet → comparison → QualityReport chain together with reference/target/camera binding before exposing `ViewerVisualEvidence@1`; missing comparison and cross-artifact relinking fail closed in Runtime, so the Viewer no longer carries the sole responsibility for reconstructing this boundary. This is a source/runtime integrity repair only; it does not change the current robot visual result or unlock human/PBR/export/restart/360 gates.

2026-08-14 Primary Form single-action transport follow-up：current-cohort receipt `real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json` verifies one coherent Runtime-owned action after aggregated observation: `primary_form_repair_prepare` performs bounded fit → Geometry Worker → strict readback → Render Worker → compare, and the CLI consumes its candidate-bound visual evidence instead of issuing a second compare. This closes the duplicate search/compare orchestration gap and records camera binding `PASS_SILHOUETTE_FIT_TO_COMPARE`; metrics remain below the visual gate (`IoU 0.749122`, `Boundary F1 0.347623`), so it is not a high-quality or benchmark PASS.

<!-- forgecad-stage0: schemas=102 schema_set_sha256=19b0b54b3f97b68fa20bd3ae64dbd4ffa22106bd8573aa5550950356fd87e668 read_tools=35 write_tools=22 total_tools=57 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json latest_completed=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json -->

本文是 MCP010A–F 的唯一详细执行合同。它不改写 MCP005–009 的历史 evidence，也不把目标 Schema、工具、Skill、库或素材写成当前能力。

ADR-0026 补充本计划的方向：MCP010F 之后不能继续只靠堆 operator/detail/material 追求高质量，必须把 `ReferenceCanvas → DesignSpec → SemanticSceneGraph → stage gates → Visual Evidence Bundle → Critic/Repair` 变成产品化 authoring loop。在对应 Schema、Runtime producer、MCP tools 和真实 Codex evidence 完成前，这些仍是目标设计。

当前已完成的 Agentic slice 包含 Runtime-owned read-only projection 与受批准的 durable prepare/readback：`scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get` 通过 source/runtime/MCP/Viewer Gate；`session_create_or_resume`、`session_get`、`checkpoint_prepare`、`checkpoint_get`、`checkpoint_restore_prepare` 通过合同/重启 receipt。后者只持久化 session/checkpoint 并生成 CAS-only RepairIntent，不执行 Repair，不解锁视觉/PBR/confirm/export；后续质量任务必须继续读取本计划的 candidate/camera/quality truth。

## 1. 目标和声明边界

把现有 primitive blockout 升级为 Codex 可驱动、可回读、可比较、可局部修改和可回退的首个白色硬表面机器人质量闭环：

```text
ReferenceEvidence
→ typed detail inventory
→ GeometryProgram/AppearanceProgram
→ mesh + UV/PBR + self-contained GLB
→ fixed RenderSet/AOV
→ reference metrics + typed visual review
→ user review/approval
→ immutable version → restore → CAS export
```

当前只有一张裁切腿脚、正面三分之四参考。因此：

- 本轨道首先验收可见视图，最高状态是 `PARTIAL_VISIBLE_VIEW_PASS`；
- 用户补充 front、back、left、right、rear-three-quarter 五张同设计全身参考之前，`HQ_360_PASS` 固定为 `BLOCKED_REFERENCE_COVERAGE`；
- 隐藏结构必须标记 `unknown/inferred`，不能以对称或想象伪装成参考事实；
- 本轨道不承诺骨骼动画、制造 CAD、工程安全、跨类别通用重建或公开发行。

当前高质量主路径固定为 `GeometryProgram@2` + active detail Operators → `ArtifactReadback@2` strict readback →（轮廓门解锁后）`AppearanceProgram@2` → `RenderSet@2` 九 AOV → candidate-bound strict `reference_compare_prepare` → typed `visual_review_submit` / `QualityReport@2`。`[transition-v1]` `GeometryProgram@1` + primitive-only + `RenderSet@1` 四 pass 只保留 MCP007–009 兼容/结构导出，不得作为当前高质量、reference likeness、PBR 或 360 路径。

未来 Agentic high-quality 主路径在上述硬门之前增加设计理解层：

```text
ReferenceCanvas@1
→ DesignSpec@1
→ SemanticSceneGraph@1 / ModelUnderstandingBundle@1
→ DesignStagePlan@1
→ single bounded action
→ strict readback / AOV / compare
→ DesignCriticReport@1
```

该路径的新增对象只能从 Runtime/Worker/Render/Quality evidence 派生；不得让自然语言、截图或外部 DCC 状态成为真值。

## 2. 当前事实与目标分离

### MCP010B/C current source reconciliation

最新校正：`d9c23b…ac0bd` 是当前安装并已由用户完整重启加载的 Skill-overlay Dev.app，已通过 package/raw/real-Codex V2 structural probes 和 live Desktop structural activation。`5143ac3b…6e61`、bfa56 与更早 cohort 保留为历史 receipt；当前 live Desktop 为 d9。

MCP006 的 44-contract、MCP010B 已保存的 50/52-contract aggregate，以及 3c/f488/bfa56/d9 Dev.app/raw/CLI receipt 都是历史或结构事实，原样保留。当前源码总计 **102 个 JSON Schema**：历史合同、MCP010B/C/D/E/F 当前合同，以及 Agentic contract family。当前 source Gate 已通过 B 的 V2 geometry/readback/Worker isolation、C 的 fixed renderer/九 AOV/reference compare/review raw path、D 的真实 Operator、E 的离线 AssetPack/UV/PBR/MikkTSpace raw path，以及 F 的哈希绑定轮廓目标、扩展相机搜索、Runtime-owned camera reference、受限 Rig/SDF fit、Runtime-owned bounded Primary Form search、单/多 Part contour proposal、Part error table、candidate compare 和 `primary_form_repair_prepare` staged prepare/evaluate path；Agentic projection 与 durable prepare/readback 也通过合同/Viewer/隔离重启 probe；当前工具面为 35 read + 22 opt-in write。C/E/F receipt 使用 synthetic 或结构性 reference，只证明绑定、传输、持久化和 deterministic bytes，不证明 PBR likeness、用户 robot likeness、Viewer/package/live、人评或 360。Agentic durable receipt 也不证明通用单动作 orchestrator、Repair execution 或视觉 PASS。下文任何未特别注明的旧“50/52/65-contract/current Dev.app”叙述都应按本段分层。

| 项目 | 当前事实 | MCP010 目标 |
| 说明 | 上一段中的 `5143ac3b…6e61` 与 bfa56 仅是历史 package/live receipts；当前安装包与完整重启后的 live cohort 以 `d9c23b…ac0bd` 为准 | live 证据仍只证明结构工具链，不提前写成视觉/PBR能力 |
|---|---|---|
| 合同 | MCP006 历史为 44 个 JSON Schema；当前 MCP010B/C/D/E/F source contracts 与 Agentic contract family 使当前 manifest 为 102 个 JSON Schema（含 `CameraCalibrationRef@1`、`SilhouettePartErrorResult@1`、`DesignSession@1`、`DesignCheckpoint@1`、`RepairIntent@1`） | 维持版本化合同；后续任务只可新增有证据的 Viewer/闭环合同 |
| MCP | 当前源码为 35 read + 22 opt-in write（57）；F 新增 Runtime-owned `silhouette_rig_hash` 以避免 Codex 本地重算 Rig canonical hash、`silhouette_part_error_get` 多 Part 误差表和 `primary_form_repair_prepare` 单动作 staged prepare/evaluate；Agentic 新增 projection tools、session/checkpoint readback 和 approval-gated prepare tools；C/E/F/Agentic source raw/restart Gate 已按各自范围通过，历史 Dev.app receipts仍按 cohort 保存 | 真实用户 likeness、同一 candidate 的 packaged Viewer、人评/PBR/360证据仍需独立 Gate；不得用 synthetic/raw、prepare receipt 或 projection 直接宣传高质量 |
| Skill | 十个历史 first-party `0.1.0` declarative Bundle + 当前 `primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`uv-pbr@0.2.0` active overlay；AssetPack 独立于 Skill | 仅在真实 consumer、bundle integrity、AssetPack provenance 和 benchmark 都通过后保持 active |
| 几何 | MCP010D source 已提供 primitive、profile/extrude、loft、revolve、tube-sweep、transform、mirror、array、panel、vent-array、joint-stack、part-output；同 cohort packaged D raw structural probe 已通过 | Manifold boolean、真实用户视觉阈值仍未运行 |
| Render | `[transition-v1]` MCP008/009 保留四个 compatibility pass；MCP010C source 已有 512×512 perspective/z-buffer + 九 AOV + local metrics；MCP010F source Viewer 与 packaged CLI read-model 可读取这些 AOV并做临时对比 | 核心控件 smoke 已运行；同一 provisional observation 的 packaged binding、正式 VoiceOver、真实用户视觉阈值、export/restart hash仍未运行 |
| 材质 | bounded glTF PBR：embedded baseColor/normal/metallic-roughness/AO/emissive channel sampling | first-party 离线 AssetPack、纹理、clearcoat/emissive strength |
| Viewer | 只读 GLB canvas/read model；compare/AOV/diff/Part/MaterialZone/explosion/heatmap source surface、packaged CLI read-model、原生窗口与核心控件 smoke 已通过 | 同一 provisional observation 的 package binding、正式 VoiceOver、真人视觉门仍独立验收 |

只有对应 producer、consumer、negative/focused/真实 Codex evidence 全部通过后，能力才能从 `planned/unavailable` 变为 `available`。

### ADR-0026 Agentic Design Loop target

下列能力是 MCP010F 后续重构目标，不属于当前 MCP010F source PASS：

| 目标能力 | 当前状态 | MCP010 后续退出要求 |
|---|---|---|
| `SemanticSceneGraph@1` / `ModelUnderstandingBundle@1` | 目标设计 | 从 candidate/readback/RenderSet/Quality 派生，返回 Part roles、dimensions、symmetry、source map、MaterialZone、camera、selection、uncertainty |
| `ReferenceCanvas@1` / `DesignSpec@1` | 目标设计 | 绑定 reference CAS hash、视图 coverage、observed/inferred/unknown、primary/secondary/tertiary goals |
| `DesignSession@1` / `DesignCheckpoint@1` | 目标设计 | Runtime-owned stage/checkpoint/rollback projection；永久写仍走 candidate/version |
| `scene_observe_get` / `visual_evidence_bundle_get` | 目标设计 | 一次返回 Codex 设计判断需要的 hash-bound 现场；默认只读 |
| Parametric Design Kit | 目标设计 | Housing/Panel/Vent/Joint/Sensor/Frame 等 intent 展开为 typed bounded Geometry/Appearance programs |
| `DesignCriticReport@1` / `RepairIntent@1` | 目标设计 | 只输出 evidence-bound 单 Part/MaterialZone repair，不直接写几何 |

这些能力完成前，当前可见视图仍按已有 `reference_mask_prepare → silhouette_target_get → scene_observe_get → camera_fit_prepare → silhouette_rig_hash → silhouette_fit_prepare → reference_compare_prepare → render_pass_get → visual_review_submit → quality_get` 链路执行；其中 `scene_observe_get` 是同一视觉回合的一次 canonical Runtime projection，不能被拆成跨轮次的零散观察。

## 3. 原子任务链

### 3.1 FGC-MCP010A — 权威重排与开发激活

Owned：权威文档、文档 checker、用户级开发 App 构建/激活、原始 stdio/CLI/真实 Codex capability evidence。

MCP010B 的 authoring 可用性补口已经在当前源码完成：公开但只读的 `operator_catalog_get` 返回与 `forgecad://operators/catalog` 完全相同的 Runtime-owned catalog，`geometry_program_hash` 由 Runtime/Worker 的同一 canonical JSON 实现校验无 hash V2 draft 并返回 compiler-owned hash。后者不编译、不创建 candidate/Job，也不写 SQLite/CAS；它已由 `catalog → hash → prepare` raw stdio、隔离 source-focused V2 structural Gate 和完整重启后的 live Desktop hash 调用验证。它不属于 010A 已安装 Dev.app 的 30-tool 或 MCP010B 早期 `3c6f59…7140` pre-graph 历史 receipt；`f4885b11…6bc1` 所记录的 package/isolated V2 semantic-Part graph raw activation 与 exact packaged Worker structural E2E 也是历史安装 cohort receipt。当前 `d9c23b…ac0bd` Dev.app 已通过 ad-hoc/package/Worker、isolated V2 raw、real-Codex structural 和用户完整重启后的 live Desktop structural 路径；live 证据仍不宣称视觉/PBR。

必须：

1. 把任务索引重排为 010A–F；同一时刻只允许一个原子任务处于 `in_progress`；
2. 从同一源码 revision 构建 `forgecad-mcp`、`forgecad-runtime`、Worker 和 Viewer；
3. 安装到 `~/Applications/ForgeCAD Runtime Dev.app`，开发期只允许本机 ad-hoc 签名；
4. Codex 用户配置指向 App Resources 中的 `forgecad-mcp`，不再引用 `forgecad-mcp-host`；仓库配置不写 token、fixture data dir、用户名或用户绝对路径；
5. 原始 MCP/CLI Gate 通过后，由用户重启 Codex；真实调用证明 `capabilities_get`、临时 `project_create`、Runtime `Ready` 和 MCP/Runtime 相同 build hash。

退出：用户重启后的真实证据必须证明工具、Runtime Ready、能力 cohort 和临时项目读回；本次已满足并将 010A 标记 `done`。不得自动领取 010B。ad-hoc 开发 App 不是 MCP013 的签名安装包。

当前进度：010A/010B 历史与 source structural evidence 见 `docs/evidence/mcp010a/`、`docs/evidence/mcp010b/`；C source evidence 见 `docs/evidence/mcp010c/`；D/E/F source evidence 见 `docs/evidence/mcp010d/`、`docs/evidence/mcp010e/`、`docs/evidence/mcp010f/`。当前源码总计 102 个 JSON Schema、35 read + 22 opt-in write = 57 个工具。用户第一次 Desktop restart 的 FAIL receipt和后续结构 PASS receipt均保持原样。C 当前源码已在隔离 raw stdio 中证明 56-tool source manifest、九 AOV、candidate-bound comparison、MCP image block、Codex typed visual review 与 deterministic bytes；E raw stdio 已证明 AssetPack manifest/provenance、embedded PNG textures、512px UV atlas、固定 mikktspace、PBR bindings 和同一九 AOV render path；F source Gate 已新增 hash-bound silhouette target、扩展 camera search、`CameraCalibrationRef@1`、Runtime-owned bounded Primary Form/SilhouetteRig/SDF fit、单 Part contour proposal、candidate compare、只读 Viewer 的九 AOV、reference/render split/overlay/flicker、Part/MaterialZone 筛选、爆炸图和热图辅助及 TypeScript/Vite/Tauri 构建，另有 packaged CLI read-model、原生窗口与核心控件 smoke；Agentic projection 与 durable prepare/readback 另通过 preflight 顺序、空 reference fail closed、合同 checker 和 Runtime/MCP 重启 probe。C/D/E/F 结构/传输证据不是用户机器人 likeness；同一 provisional observation 的 packaged Viewer 绑定、正式 VoiceOver、独立人评阈值、xatlas/Validator、真实 PBR likeness、export/restart hash 和 360仍 `NOT_RUN/BLOCKED`。短时 launcher flock 只用于启动选主，Runtime `runtime.writer.lock` 才是最终唯一写者。

### 3.2 FGC-MCP010B — V2 合同与几何真值

兼容界限：`[transition-v1]` `GeometryProgram@1` 继续服务已存在的 MCP007–009 primitive-only appearance/export MVP 路径，且 Runtime 现会对其 GLB 作物理回读；它不是 MCP010B 的 V2 high-quality 写路径，也不得借此获得 V2 catalog、strict `ArtifactReadback@2`、九 AOV strict compare 或材质声明。历史对象不迁移、不改写。V1 新写入口的最终移除须与 MCP010E 的 `AppearanceProgram@2` 迁移一并设计，不能在 B 中让当前已验收的 V1 appearance/restore/export 链静默断裂。

Owned：`GeometryProgram@2`、`OperatorCatalog@1`、`ArtifactReadback@2`、GLB/accessor validator、primitive 修复及负向 fixture。

当前 B source Gate 与 C/D/E/F source Gate 已通过：B 覆盖 V2 geometry/readback/Worker isolation/restore；C 覆盖当前合同 checker、固定 renderer、九 AOV、local mask/metrics、candidate-bound review、MCP image block 和 deterministic raw stdio（C 历史 subtotal 为 59）；D 覆盖 13-entry catalog/12 active operators；E 覆盖离线 AssetPack、512px UV atlas、fixed mikktspace、embedded PBR/九 AOV；F 覆盖哈希绑定轮廓目标、37 个覆盖全局尺度的粗候选加 9 个局部探针、扩展 Rig/SDF 搜索、单 Part proposal、2–8 候选比较、方向性边界误差、只读 Viewer 的 AOV/compare/Part/MaterialZone/explosion/heatmap source surface、TypeScript/Vite/Tauri 构建和 write-boundary negative check。Agentic projection 与 durable session/checkpoint/RepairIntent prepare/readback 另有独立合同/重启 Gate。当前 source tool manifest 为 35 read + 22 opt-in write = 57。历史 package/CLI/live receipts保持原样；C/D/E/F/Agentic synthetic/raw/source不等于真实 robot likeness、PBR likeness、同一 candidate 的 packaged Viewer、人评或 360。Darwin OS total-memory hard cap、xatlas、Khronos Validator仍 `NOT_RUN`；授权单图仍只能产生 `PARTIAL_VISIBLE_VIEW_PASS`，HQ_360 仍 `BLOCKED_REFERENCE_COVERAGE`。

必须：

- `GeometryProgram@2` 按 `operator_id` 使用封闭参数 Schema，真实 DAG inputs，米/弧度单位，显式 Part outputs、operator catalog hash 和完整预算；
- Codex 必须先用 `operator_catalog_get` 读取 live `OperatorCatalog@1`，把同一 digest 写入 hash-free draft，再调用 `geometry_program_hash`；返回 hash 填回 program 后才可 `geometry_prepare`。`GeometryProgram@2.project_id` 必须与外层 target project 完全相同：hash/catalog mismatch 由 hash/compile validation 拒绝，project mismatch 由 `geometry_prepare` 在编译和持久化前拒绝；
- V2 物理 envelope 固定为：position 各轴 `[-10, 10]` m，box `size_m` 与 cylinder `height_m` 为 `(0, 10]` m，sphere/cylinder radius 与 ellipsoid radii 为 `(0, 5]` m，rotation 各轴为 `[-2π, 2π]`；
- 修复 sphere 极点退化、cylinder 端盖法线、ellipsoid 法线、UV/tangent 假 PASS；
- Runtime 遍历 GLB BIN/accessor，真实计算 index/non-finite/degenerate/boundary/non-manifold/winding/Part/Material/source coverage；
- 删除 hard-coded validator PASS；损坏 index、source map、hash、winding 或 UV 时 fail closed；
- `[transition-v1]` `@1` 不迁移或改写已确认版本；MCP007–009 的 primitive-only 兼容链只保留历史结构/导出用途，不能产生 V2、detail、九 AOV strict compare 或高质量结论。

预算：512 nodes、250k triangles、64 MiB candidate GLB、512 MiB Worker memory；单次编译目标 10 秒以内。当前 macOS 实证已证明 10 秒墙钟超时/回收、受限 Rust allocator guard 和 `wait4` 峰值 RSS 后验拒绝；但 `RLIMIT_AS`/`RLIMIT_RSS`/`RLIMIT_DATA` 不能提供本机可证明的预防式硬上限，不能把 512 MiB 写成 OS 总内存硬上限；该子门保持 `NOT_RUN`，测量失败不能转移为 MCP011 全局性能实现。

### 3.3 FGC-MCP010C — 固定渲染与参考比较

Owned：`ReferenceViewSpec@1`、`CameraCalibration@1`、`RenderSet@2`、`ReferenceComparisonReport@1`、`VisualReviewReport@1`、`HumanVisualReviewReceipt@1`、`QualityReport@2` 和四个 MCP 工具变化。

Renderer 必须提供 512×512 perspective、真实 camera transform、z-buffer、确定性抗锯齿、固定 GGX 直接光和显式色彩管理；同一 candidate hash 输出：

1. beauty；
2. silhouette；
3. depth；
4. normal；
5. AO；
6. part-ID；
7. material-ID；
8. wireframe；
9. UV-stretch。

目标工具：

| 工具 | 类型 | 行为 |
|---|---|---|
| `render_pass_get` | read | 返回 hash-bound PNG image block；不生成新 render |
| `reference_compare_prepare` | write/temporary | 生成 camera、mask、metrics 和 diff，不创建版本 |
| `visual_review_submit` | write/evidence | 保存 Codex 对具体 pass/region 的 typed issue |
| `human_visual_review_submit` | write/evidence + confirmation | 保存用户评分，不作为密码学身份认证 |
| `quality_get` | existing read | 只读取 Runtime 已持久化且绑定当前 hash 的报告 |

当前源码工具数量为 35 read + 22 opt-in write = 57。MCP010A/010B 的 30/32-tool Dev.app receipts均为历史 structural cohort；C source raw 已证明 `render_pass_get` image block 和三项视觉证据工具，D 当前 packaged raw 已证明同 cohort Operator/strict readback transport，E raw 及当前 packaged E 已证明 `material_pack_get`、embedded texture 和九 AOV render path，F source 已证明轮廓目标、37 个覆盖全局尺度的粗候选加局部探针相机拟合、`CameraCalibrationRef@1`、Rig/SDF/Part/candidate compare、边界误差读取和 `primary_form_repair_prepare` staged prepare/evaluate；Agentic projection 与 durable prepare/readback 已通过合同 checker、preflight 顺序、空 reference fail closed 和 Runtime/MCP 重启 probe。packaged Viewer 已有 read-model/window/core-control smoke，但与 attempt35 provisional observation 的 package binding、正式 VoiceOver、真实用户 likeness/PBR likeness、人评阈值和所有 360° evidence仍 `NOT_RUN/BLOCKED`。

参考 mask 使用产品内确定性 border flood-fill/morphology；Codex 提交 normalized landmarks、region、visibility 和 unknown/inferred。每个 candidate 最多五轮 `silhouette → structure → form → material/surface → final` 修正；未达标返回 `QUALITY_TARGET_NOT_MET`，不能自动 confirm。

区域质量门必须在同一 declared region 内比较 reference/model mask；不得把整个模型 mask 与区域矩形直接做 IoU。Runtime 的 `region-mask-iou-v2` 实现已采用该定义，并排除 `unknown` 区域；它修正指标真值，但不放宽 silhouette、boundary、landmark 或 human gate。

为减少每轮上下文和截图噪声，C/F 允许使用 `scripts/make_mcp010f_comparison_sheet.py` 将同一 reference、beauty、silhouette 和一个 diagnostic AOV 打包为固定 2×2 review sheet。它是标准库 review helper，只保存 hash-only manifest，不计算质量、不写 Runtime/CAS；原图含用户字节时必须留在临时目录。Runtime `QualityReport@2` 与 candidate-bound comparison 仍是唯一质量真值。

F 还提供 `scripts/build_mcp010f_fit_plan.py` 作为本地 Codex 编排辅助器。它验证 `ReferenceComparisonReport@1`、`ReferenceViewSpec@1` 和可选 `OperatorCatalog@1` 的 canonical hash，按五个有序阶段生成最多五条单一 Part/MaterialZone 修正意图，并保留每条意图的 metric、landmark、region 与 observed/inferred/unknown 来源。当前输出还为已知 region 提供稳定的 `primary_part_ids`、只读 `supporting_part_ids`、`material_zone_hints` 和按 Part 分组的 `part_operator_hints`；每轮只保留一个主 Part，未知 region 进入 `unmapped_region_ids`，不会被猜成可执行部件。它不写 Runtime/CAS、不生成几何参数、不调用 Operator、不替代 `QualityReport@2`；缺少活动目录时会清空 `operator_hints` 并明确记录阻断原因。该输出只能留在临时目录或脱敏后作为编排证据。

### 3.4 FGC-MCP010D — 受限高细节几何（source-focused PASS）

Owned：真实 Operator consumer、Worker 隔离/预算、Operator catalog、geometry Skills `0.2.0` 和 Manifold adoption。

目标 Operator：

- `primitive@2`：rounded-box、正确 cylinder/sphere；
- `profile-extrude@1`、`profile-loft@1`、`revolve@1`、`tube-sweep@1`；
- `transform@2`、`mirror@1`、`array@1`；
- `boolean@1`：只允许同一 Part scope 的 union/difference；
- `panel@1`、`vent-array@1`、`joint-stack@1`；
- `part-output@1`：一个语义 Part 可由多个细节节点组成。

Manifold 固定目标为 v3.5.2，仅 C API 静态进入隔离 geometry worker，关闭 Python/JS binding、自动下载和不受控并行。采用前必须有 full revision、LICENSE hash、transitive SBOM、恶意输入/时间/内存/确定性/source-ID benchmark 和 removal plan；receipt 未 `accepted` 时 `boolean` 保持 unavailable，机器人使用分层 shell，不阻塞其他 Operator。

当前实现结果：`operator_catalog_get` 返回 13 项，其中 12 项 active（primitive@2 加 11 个 D Operator），`boolean@1` 明确为 `unavailable`；`script/test_mcp010d.sh` 已通过 contracts、source-built Worker/Runtime/MCP、raw stdio `catalog → hash → prepare → readback`、7 semantic Parts/464 triangles、strict lineage、determinism、future-input/unknown-parameter/boolean negative。`hard-surface-detail@0.2.0` 的 manifest、recipe、operator lock、benchmark fixture、LICENSE/NOTICE、SPDX SBOM、provenance 和 development trust 均通过 Runtime integrity 后才返回 `active`。证据：`docs/evidence/mcp010d/manifest.json`、`focused-gates.json`、`raw-stdio.json`。

本次 source-focused 退出不包含 Manifold adoption：未锁定 v3.5.2/full revision、未进入 C API Worker、未运行其恶意输入/内存/确定性/source-ID benchmark，因此 boolean 保持 unavailable。当前 Dev.app D 已有同 cohort raw structural receipt（catalog → GeometryProgram@2 → strict readback），但不证明 Viewer presentation、真实 Codex Desktop D 或视觉 likeness；真实 PBR likeness/纹理审美、第三方 Validator 和 360°也不由本节推导，均继续记录为 `NOT_RUN/BLOCKED`。

机器人需要稳定 head/visor/neck、chest/core/shoulder、arm/hand、pelvis/hip、thigh/knee/shin/ankle/foot，并包含可追踪 panel、vent、joint ring、cable、emissive housing；左右结构使用 mirror，不维护两套漂移参数。

当前用户单图 source benchmark 的最新几何/表面改进包括四层：panel@1 的固定四段圆角 profile 把面板从平面八点倒角升级为可重复的圆角轮廓；在其上使用 panel/mirror/vent/joint 组成 visor-edge、chest-ridge、shoulder-trim、forearm-rail、hip-flank、knee-cap 六类表面线流层；AppearanceProgram@2 将这些 semantic Parts 绑定到离线 AssetPack MaterialZone，当前保留两个可审计配方：8-zone surface-zones 用全套材质族，7-zone armor-shell-zones 保留可见上臂/前臂白色外壳并把深色限定在内构、凹槽、线缆和发光通道；profile-loft/revolve/tube-sweep/joint-stack 对重合且法线相容的曲面顶点启用受限平滑法线，但保留 panel/cap 锐边。当前 material-zoned linework receipt 为 26 Parts/4704 triangles，silhouette IoU 0.7410、boundary F1 0.3288、region median IoU 0.8694、critical-region minimum 0.6663；相对 rounded-panel baseline 的变化指标全部改善或持平，但仍低于整体视觉门，不能 confirm/export。rounded-panel、linework、surface-zones 和 armor-shell-zones receipts 保留在 `docs/evidence/mcp010f/`，旧 3368-triangle receipt 仍为历史对照；后续几何修正仍必须先做单一 Part 变更再跑同一 candidate 的 comparison，材质区增多不能替代轮廓修正。

### 3.5 FGC-MCP010E — 离线 AssetPack、UV 与 PBR

Owned：`MaterialPackManifest@1`、`MaterialDefinition@1`、`TextureSet@1`、`TextureBuildReceipt@1`、`AppearanceProgram@2`、first-party AssetPack、bounded UV atlas、固定 `mikktspace@0.3.0` tangent producer、glTF Validator deferred adoption 与材质 Skills `0.2.0`。

当前 E source 与 packaged structural 事实：固定 beauty renderer 已读取嵌入 baseColor/normal/metallic-roughness/AO/emissive 纹理，并以固定 key/fill/rim GGX-like 光照、clearcoat 与 emissive strength 参与采样；最新 PBR renderer Dev.app cohort `77d4bff5…f2a73` 已完成 ad-hoc/package/隔离用户参考图探针，但本次未重启 Desktop，且比较结果仍 `QUALITY_TARGET_NOT_MET`。这证明通道接线和离线 provenance，不证明机器人 PBR likeness、色彩审美、人评、export/restart hash 或 360°。最新胸甲浅斜切实验也只作为负向视觉证据：boundary F1/全局 silhouette 小幅改善，但 landmark/region 覆盖下降；单独增加胸甲上缘 cap 仍未改善全局 silhouette，retained baseline 未改变。

`forgecad-hard-surface-robot@1.0.0` 必须是 AssetPack，不是 Skill，包含：白色 dielectric clearcoat、深灰喷涂金属、黑色阳极氧化金属、拉丝钢、工程塑料、关节橡胶、暖橙 emissive 和微划痕 normal/roughness 层。

实施期只允许 Codex 一次性下载以下免费 CC0 文件到本机 adoption cache，不调用 API：

- ambientCG `Metal010` 2K PNG；
- ambientCG `Plastic006` 2K PNG；
- Poly Haven `Studio Small 03` HDRI。

每个原文件记录 source URL/ID、retrieved_at、SHA-256、作者、SPDX、license text hash、通道/色彩空间和处理 Recipe。原 ZIP 不进入 Git；派生 `.forgecad-material-pack` 经 manifest 校验后进入开发 App Resources，Runtime 首次启动写 CAS，运行时永不联网。

UV/tangent/GLB 规则：

- UV atlas 当前使用 ForgeCAD 自有的 512px deterministic triangle-chart grid（每 chart 4 texel padding、无重叠、finite/zero-area 回读）；xatlas 仍只保留 approved-for-evaluation，未进入产品依赖；
- `mikktspace@0.3.0` 固定 crates.io checksum 与 GitHub revision，通过 `docs/evidence/adoption/mikktspace/0.3.0.yaml` 的 license/SBOM/恶意输入/确定性 receipt 后进入受限 Geometry Worker；Runtime 不直接链接该库；
- baseColor/emissive 为 sRGB，normal/metallic/roughness/AO 为 linear，normal 固定 OpenGL `+Y`；
- GLB 内嵌 PNG、禁止 external URI、按材质 hash 去重；candidate texture ≤64 MiB，export ≤128 MiB；
- 支持 ratified `KHR_materials_clearcoat`、`KHR_materials_emissive_strength`；KTX2/LOD/通用 pack installer 延后；
- glTF Validator 是开发 Gate，不能替代 Runtime readback。

升级 `uv-pbr@0.2.0`、`render-evidence@0.2.0`、`reference-compare@0.2.0`；当前 100-contract checker、AssetPack manifest/provenance、Worker/MCP raw Gate 和同 cohort packaged E 结构性用户参考探针已通过。Khronos Validator、真实 PBR likeness、export/restart hash、同一 provisional observation 的 packaged Viewer binding 和 Viewer 人评仍 `NOT_RUN`；未来若第三方 adoption 失败，必须回退到 product-owned strict readback，而不能制造 Validator/mikktspace PASS。

### 3.6 FGC-MCP010F — Viewer 与真实机器人闭环

Owned：reference/render split、overlay、flicker、diff heatmap、九 AOV、camera lock、Part/MaterialZone selection/isolate/explosion、candidate undo/redo、Viewer a11y 和真实 Codex/human evidence。

当前 source slice：`script/test_mcp010f.sh` 已通过 Viewer source checker、TypeScript/Vite build、Tauri workspace compile、read-only IPC/write-boundary negative check；同 cohort Dev.app 的 `--viewer-read-model` 也已在隔离用户参考 candidate 活跃期间返回 `ForgeCADViewerReadModel@1` Ready 投影，artifact/quality/reference 结构映射通过。另有一次隔离 Vite browser DOM smoke 实际点击并验证了 9 个 AOV、3 种比较模式、轮廓画布、差异热图和 flicker；无 Runtime 数据时阶段保持 `reference-canvas` 且 correction queue 为空；同 cohort Dev.app 的 frontmost native-window smoke 观察到 1440×891 的 `ForgeCAD Runtime Viewer`，而本轮 Computer Use 又从打包 WebKit AX 树实际操作 AOV、Home/End、overlay/flicker、轮廓画布、差异热图和爆炸图。System Events 仍未暴露 WebKit 子树，故正式 VoiceOver/无障碍与人评仍保持独立 NOT_RUN。`RuntimeViewer.tsx` 的选择、材质区筛选、临时爆炸图、差异热图、显式 contour canvas、ephemeral reference-contour aid、contour-first 阶段门和 Codex correction queue 都只修改 ephemeral UI state；轮廓画布现在复制 candidate-bound `ForgeCADViewerContourDraft@2`，由 `scripts/validate_mcp010f_contour_draft.py` 生成单 Part `ForgeCADContourCorrectionIntent@1`，并拒绝 stale hash、自交/越界点和未选 Part 的写入意图。轮廓画布只是一键选择既有 silhouette AOV 与 overlay，reference-contour aid 使用与 Runtime `mask-2` 同源的受限边界连通 flood-fill/局部颜色差规则，仍只是视觉辅助，阶段门与 correction queue 仅从 candidate-bound metrics 生成提示，二者都不进入 QualityReport。视觉解锁只读取 candidate-bound `QualityReport@2.visual_status + hard_gate_passed`，不会把结构 candidate 的 `quality_hard_gate_passed` 当成视觉通过。Correction queue 不携带几何参数，只允许下一轮单 Part/单 MaterialZone 的受限意图，且要求保持 camera/reference/hash 不变并重新回读。当前仍未运行正式 VoiceOver accessibility、真人评分、export/restart hash 和五视图 360 门，因此 F 仍为 `in_progress`，不能宣称视觉质量完成。

实际闭环演练：在打包 Viewer 选择 `chest-shell` 并绘制 candidate-bound 轮廓后，Codex 将草图限制为单 Part `chest-wedge-mild`，在隔离 Runtime 中重新执行几何、材质、参考比较和质量读取。该次传输/回读通过，但全局质量门仍失败（silhouette IoU 0.7399、boundary F1 0.3263、landmark coverage 0.6667、region median IoU 0.8487），相对 26-Part/4704-triangle 保留基线退化，未晋级、未 confirm/export。证据 `docs/evidence/mcp010f/contour-execution-actual-20260812.json`；此处证明的是 Viewer→Codex→Runtime 的实际单部件迭代链路，不是视觉相似度通过。

2026-08-13 F 顺序与局部修正证据：每个新的设计 MCP session 先读取并校验 `ponytail-preflight@0.1.0`，再进入 project/reference/operator/geometry 调用；`scripts/probe_mcp010f_part_correction.py` 的静态顺序 Gate 已覆盖该要求。真实隔离探针随后读取 `chest-shell` Part 误差表，执行五个有界单部件候选并完成 candidate-bound comparison；receipt `docs/evidence/mcp010f/part-correction-source-20260813-followup.json` 的最佳 silhouette IoU 为 `0.745895`、Boundary F1 为 `0.330265`，仍为 `QUALITY_TARGET_NOT_MET`。这只是 ordered transport/局部修正证据，不是 likeness、confirm、export 或 packaged Viewer 质量证据。

同轮又把 probe 的受限路由扩展到 `shoulder-armor-left/right`，并以 `shoulder-contour-mild` 的 `shoulder-armor-right` 和用户授权参考右肩 contour 执行五候选比较。Runtime 成功返回局部 Part-error 与 `shoulder-width/height/offset` proposal，但最佳全局 silhouette IoU `0.744471`、Boundary F1 `0.327606` 仍未达到门；receipt `docs/evidence/mcp010f/part-correction-source-20260813-shoulder-right.json` 只证明多 Part attribution/ordered transport，不是 likeness、confirm、export 或 packaged Viewer 质量证据。

随后同一闭环选择 `shoulder-armor-left`，使用图像派生左肩 contour 完成局部 proposal 与五候选 compare；最佳 silhouette IoU `0.742468`、Boundary F1 `0.327530`，未改善肩甲基线，receipt `docs/evidence/mcp010f/part-correction-source-20260813-shoulder-left.json` 只作为 `QUALITY_TARGET_NOT_MET` 的负向单 Part 设计证据保留。不得因此进入材质、确认、导出或 360 门。

Viewer 只有一个 WebGL context，继续只读 Runtime projection。永久 geometry/material/restore/export 仍回到 Codex 的 prepare/approval/confirm。

真实链路固定为：

```text
reference_import → operator_catalog_get → geometry_program_hash(GeometryProgram@2/detail)
→ geometry_prepare → artifact_readback_get(ArtifactReadback@2 strict)
→ reference_mask_prepare → camera_fit_prepare / silhouette_fit_prepare
→ appearance_prepare(AppearanceProgram@2，仅在前序门解锁后)
→ reference_compare_prepare(同一 camera/reference/candidate，九 AOV strict compare)
→ render_pass_get × 9 → visual_review_submit → quality_get
→ 最多五轮单 Part change_prepare / strict compare
→ human_visual_review_submit（独立真人门）
→ candidate_confirm → version_diff
→ restore_prepare/restore_confirm
→ export_prepare/export_confirm
```

输出必须有 self-contained GLB、固定视图、diff、QualityReport、human receipt、immutable version、restore/export receipts，以及 Viewer/export/restart 同 hash 证据。

## 4. 质量门

### 4.1 几何/UV/PBR 硬门

- invalid index、non-finite、超阈值 degenerate triangle 为 0；
- 声明 solid 的 Part boundary/non-manifold edge 为 0；
- triangle 100% 绑定 `part_id + source_node_id + material_zone_id`；
- 同机器重复五次 program/mesh/GLB/report hash 一致；
- MaterialZone binding 完整，无 unused/unknown；
- UV finite、零面积 UV triangle 为 0，padding/density 满足 Recipe；
- tangent orthogonality/handedness/normal convention 通过；
- GLB 无 external URI，Runtime readback 与 Khronos Validator 0 error；
- restart/restore/export 后 pack/material/texture/GLB hash 不变。

### 4.2 当前可见视图门

- silhouette IoU ≥0.90；
- boundary F1（4 px）≥0.90；
- bbox 边缘平均误差 ≤2%，centroid 误差 ≤2%；
- 可见 landmark coverage ≥80%，weighted NME ≤3%；
- region median IoU ≥0.85，critical region 不低于 0.85；
- 用户对 likeness、geometry detail、material fidelity、editability 各评分 ≥4/5。

指标必须记录 reference/camera/mask/render/toolchain hash、阈值、实测值和 limitation。通过这些门只允许 `PARTIAL_VISIBLE_VIEW_PASS`。

## 5. MCP011–013 保留边界

- MCP011：持久 Job checkpoint、复杂并发/cancel race、kill-9、GC/reachability、全局配额和性能；
- MCP012：通用第三方 Skill/AssetPack publisher、安装/禁用/升级/回滚、签名和撤销；
- MCP013：Developer ID、hardened runtime、notarization、clean install、正式 Codex 配置、packaged Desktop/CLI E2E、filesystem/package export、升级失败回滚和跨类别真人质量。

MCP010 的单操作预算、first-party 固定 AssetPack、ad-hoc 开发 App 和当前机器人用户评分不得替代上述任务。

## 6. 每任务验证与证据

每个子任务先记录 dirty baseline，再按 `Schema/negative → Core/Worker → Runtime/MCP/Viewer → focused → aggregate → real Codex/visual/human` 顺序运行适用 Gate。共同命令：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
npm run contracts:check
git diff --check
```

Evidence 目录为 `docs/evidence/mcp010a/` 至 `mcp010f/`。每个 manifest 分别记录 PASS、FAIL、BLOCKED、NOT_RUN、命令 exit、contract/build/dependency/artifact hash 和脱敏检查；不得修改 MCP005–009 原始 receipt。

## 7. 禁止项

- 不内置模型、Provider、付费 API、远程 image-to-3D 或素材 API；
- 不安装 BlenderMCP、FreeCAD MCP、任意 Python/JavaScript/shader 插件或 GitHub Skill pack；
- 不让 `.blend`、Three.js scene、外部 validator、截图或自然语言成为产品真值；
- 不在 010A 提前增加当前 Schema/tool/Skill 数量；
- 不在缺少多视图时宣称 360，不在单个机器人通过后宣称通用高质量；
- 不 commit、merge 或 push，除非用户另行明确要求。
