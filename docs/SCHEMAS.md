# ForgeCAD Runtime Schema 规范

版本：2026-08-13
状态：MCP005–MCP009 functional core 已落地；MCP006 历史 receipt 为 44 个 JSON Schema；MCP010B/C/E/F source 合同和 Agentic Design Runtime contract family 已进入当前 manifest，当前源合同总数为 102。唯一 `in_progress` 为 `FGC-MCP010F`；历史 package/live receipt 仍按 cohort 单独保存。Agentic observe/plan 的真实 Runtime 嵌套只读 projection 已通过 producer/consumer conformance checker；durable session/checkpoint/RepairIntent prepare/readback 与窄范围 Primary Form action-run/readback 已各自有 source/runtime/MCP/隔离证据，但不等于 durable/reference/DesignSpec 完整 producer conformance、完整 Visual Evidence conformance、通用 orchestrator 或 Repair 应用。

Stage 0 机器真值为 `docs/evidence/mcp010f/current-benchmark-truth.json`；当前源码口径同时固定为 102 Schema、36 read + 23 opt-in write = 59，并绑定 102 个 Schema 文件内容集合哈希。attempt35 只是 provisional retained observation，为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`，packaged Viewer binding 为 `PASS_CURRENT_COHORT_BOUND_READ_MODEL`（不等于 attempt35 same-observation E2E）。Schema/producer 已实现不能补齐缺失 receipt 字段，也不能越过 PBR likeness、正式真人、export/restart 或 360 门。

<!-- forgecad-stage0: schemas=102 schema_set_sha256=8d3644fb8169157584cf844a21588d4b6a49c7852600de4698205a7da6050cdb read_tools=36 write_tools=23 total_tools=59 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json latest_completed=real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json -->

## 1. 唯一来源

新合同源位于 `packages/forgecad-contracts/schemas/**`。MCP003 已验证首批 15 个 JSON Schema；MCP004 增加审批、候选、restore 和 diagnostic export records；MCP005 增加 reference admission/get records；`[transition-v1]` MCP006–009 已落地 `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1`、`RecipePlan@1`、`ArtifactReadback@1`、`RenderSet@1`、`QualityReport@1`、`ChangePrepareResult@1`、GLB export profiles 和 Skill manifest/list/get/receipt/eval records，共 44 个历史 JSON Schema。MCP010B/C/D/E/F 与 Agentic contract family 继续增加当前 V2/evidence/target/camera/Rig/fit/Part/candidate compare/session/checkpoint/RepairIntent/action-run Schema，当前 manifest 共 102；这不改写 44-contract 历史 receipt。全部 Schema 均须可解析、带 draft/id、contract manifest 为 `forgecad-runtime-contracts@1` 且声明 `model_calls=false`，manifest 与目录无漂移。Rust records 由 `forgecad-contracts` 维护；完整生成器、TypeScript 生成和额外 transport/未来宿主 conformance 仍未完成。旧 Concept/Weapon/Provider/Agent Schema 已删除。

## 2. 首批 Schema

### Runtime/MCP

`RuntimeCapabilities@1`、`RuntimeTool@1`、`RuntimeProject@1`、`RuntimeSnapshot@1`、`RuntimeJob@1`、`RuntimeError@1`、`RuntimeJobEvent@1`、`RuntimeResourceContents@1`、`Selection@1` 已落地；MCP annotations/resources Schema snapshot 在 `docs/evidence/mcp003/`。

### Project/Version

`Project@1`、`ActiveDesignSnapshot@1`、`Candidate@1`、`DesignAssetVersion@1`、`CasObject@1`、`AuditEvent@1`、`SemanticChangeSet@1`、`ApprovalReceipt@1`、`ExportManifest@1`。

### Reference/Design/Geometry/Appearance（分阶段落地）

MCP005 已落地 `ReferenceEvidence@1` 和四个 reference import/get request/result 合同；MCP006 已落地 `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1` 和 `RecipePlan@1`。`[transition-v1]` 这些 `@1` 几何/外观合同只保留历史结构兼容；当前 high-quality authoring 采用 `GeometryProgram@2` detail、`ArtifactReadback@2` strict readback、`AppearanceProgram@2`、`RenderSet@2` 九 AOV 和 candidate-bound strict compare。Agentic 的 `DesignSession@1`、`DesignCheckpoint@1`、`RepairIntent@1` 等公开合同由 Runtime 受限 prepare/readback slice 使用，内部 SQLite 记录不作为新的几何真值。

### Evidence/Skill（MCP006–009 已落地合同，执行证据仍分层）

MCP006 已加入 `ArtifactReadback@1`、`RenderSet@1`、`QualityReport@1`、`SkillBundleManifest@1`、`SkillListResult@1`、`SkillGetResult@1`、`SkillExecutionReceipt@1`、`SkillEvalReport@1`，MCP009 加入 `ChangePrepareResult@1`、GLB export profile 和 limited quality projection；`RecipePlan@1` 的单位/坐标/确定性顺序/max_edges 是显式合同。`SkillGetResult@1` 现内联 hash-bound `SkillKnowledge@1`（overview/constraints/examples），使 Codex 可在不读本机路径的情况下读取 first-party `ponytail-preflight@0.1.0`。MCP010C 已实现 `VisualReviewReport@1`、landmark/region metrics、九 AOV compare 及 Codex/human review 合同/工具接口；attempt35 的 Codex typed review 已运行但需要修订，独立真人 receipt 仍 `NOT_RUN`。完整生产 export/restart 与发布仍是后续工作，不得用空 Schema 或已存在接口代替。

## 2.1 MVP 落地顺序

| Task | 新增合同 |
|---|---|
| MCP005 | `ReferenceEvidence@1`、`ReferenceImportRequest/Result@1`、`ReferenceGetResult@1`（已完成） |
| MCP006 | `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1`、`RecipePlan@1`、MVP Skill manifest/receipt |
| MCP007 | `GeometryProgram@1`、`GeometryPrepareResult@1`、`ArtifactReadback@1`、Part/source map、worker compile request/result |
| MCP008 | `AppearancePrepareResult@1`、`RenderSet@1`、GLB UV/tangent/PBR readback |
| MCP009 | `QualityReport@1`、`ChangePrepareResult@1`、`VersionDiff@1` projection、`ExportManifest@1` `mvp-glb` profile |
| MCP010B | `GeometryProgram@2`、`OperatorCatalog@1`、`GeometryProgramHashRequest@1`、`GeometryProgramHashResult@1`、`ArtifactReadback@2`、`GeometryPrepareResult@2`、`GeometryQualityReport@2`、`GeometryCandidateEvidence@1` |

不能一次加入全部空 Schema 再宣称能力存在；每项必须与 validator、negative tests 和实际 producer/consumer 同任务落地。

## 2.2 MCP010 与 Agentic 合同（当前源合同 100；历史 44 receipt 不改写）

| Task | 目标合同 | 激活条件 |
|---|---|---|
| MCP010B | `GeometryProgram@2`、`OperatorCatalog@1`、`GeometryProgramHashRequest@1`、`GeometryProgramHashResult@1`、`ArtifactReadback@2`、`GeometryPrepareResult@2`、`GeometryQualityReport@2`、`GeometryCandidateEvidence@1` | 当前源码的 producer/consumer、真实 GLB JSON/BIN/accessor readback、closed GLB profile、V2 restore hardening 和损坏输入负向 Gate 已通过；当前 `d9c23b…ac0bd` Dev.app 已通过 ad-hoc/package、隔离/raw、真实 Codex CLI structural 和完整重启后的 live Desktop structural Gate |
| MCP010C | `ReferenceViewSpec@1`、`CameraCalibration@1`、`RenderSet@2`、`ReferenceComparisonReport@1`、`VisualReviewReport@1`、`HumanVisualReviewReceipt@1`、`QualityReport@2` | perspective/z-buffer renderer、九 AOV、metric/review persistence、tool E2E |
| MCP010E | `MaterialPackManifest@1`、`MaterialDefinition@1`、`TextureSet@1`、`TextureBuildReceipt@1`、`AppearanceProgram@2`、`AppearancePrepareResult@2` | AssetPack/UV/tangent/PBR producer、逐资产 provenance、GLB readback |
| MCP010F | `ReferenceMaskPrepareResult@1`、`SilhouetteTarget@1`、`CameraFitResult@1`、`CameraCalibrationRef@1`、`SilhouetteRig@1`、`SilhouetteRigHashRequest@1`、`SilhouetteRigHashResult@1`、`SilhouetteFitIntent@1`、`SilhouetteFitResult@1`、`PartContourFitResult@1`、`SilhouettePartErrorResult@1`、`SilhouetteCandidateCompareResult@1`、`BoundaryErrorResult@1`、`PrimaryFormAcceptance@1`（嵌入 `PrimaryFormRepairPrepareResult@1`） | Runtime-owned target/mask、bounded camera/Rig fit、hash-only calibration reference、single/multi-Part contour attribution、same-camera source/proposal retention and candidate compare |

工具 request/result Schema 随各自 producer 同任务增加；实际 manifest 数量只能从目录和 contract manifest 计算，不能把上表简单相加后提前写成当前总数。`@1` 历史版本继续只读，破坏性变化不得回填旧对象。

当前 high-quality contract path 固定为 `GeometryProgram@2` detail → `ArtifactReadback@2` strict readback → `AppearanceProgram@2`（受前序门控制）→ `RenderSet@2` 九 AOV → `ReferenceComparisonReport@1` strict compare → `VisualReviewReport@1` / `QualityReport@2`。`[transition-v1]` `GeometryProgram@1` primitive-only、`AppearanceProgram@1` 与 `RenderSet@1` 四 pass 只属于历史兼容，不得提升为当前 high-quality contract path。

## 2.3 ADR-0026 合同与当前落地层级

Agentic contract family 已进入当前 manifest 并通过正/负 fixture checker，但必须区分“合同定义”和“producer conformance”。当前 Runtime 同时提供 `AgenticSceneObserveResult@1` 可重建只读 envelope，以及受批准的 `DesignSession@1`/`DesignCheckpoint@1`/`RepairIntent@1` prepare/readback slice。真实 Runtime 产生的 `AgenticSceneObserveResult@1` 与 `DesignStagePlanProjection@1` 嵌套只读投影已由 `scripts/check_agentic_projection_receipt.py` 对隔离回执完成 producer/consumer 校验；durable 对象已经在 Runtime SQLite/CAS 持久化并经 Runtime/MCP 重启 receipt 校验，但不代表 durable/reference/DesignSpec 完整 producer conformance、单动作 orchestrator 或 Repair 应用。隔离证据见 `docs/evidence/mcp010f/agentic-runtime-observe-plan-20260813.json`、`docs/evidence/mcp010f/agentic-runtime-projection-conformance-20260813.json` 与 `docs/evidence/mcp010f/agentic-runtime-session-checkpoint-20260813.json`。

| 目标合同 | 用途 | 激活条件 |
|---|---|---|
| `DesignSession@1` | 当前设计会话、stage、candidate/checkpoint binding、失败门 | Runtime producer、MCP read surface、negative tests 和真实 Codex evidence |
| `DesignCheckpoint@1` | stage checkpoint、rollback/restore intent、candidate/version refs | 不移动 confirmed head；必须绑定 CAS/quality hash |
| `DesignStagePlan@1` | 当前允许动作、禁止动作、下一步单 Part/MaterialZone intent | 只读工具先行；不得创建 geometry |
| `ReferenceCanvas@1` | multi-view reference coverage、observed/inferred/unknown、camera claims | 绑定 `ReferenceEvidence` CAS hash；缺视图阻断 360 |
| `DesignSpec@1` | category/style/primary forms/semantic parts/material language/stage criteria | Codex 生成草案，Runtime 校验范围和 hash |
| `SemanticSceneGraph@1` | part tree、role、dimensions、symmetry、source map、editability | 从 candidate/readback/source map 派生 |
| `ModelUnderstandingBundle@1` | SceneGraph + geometry stats + material zones + cameras + AOV/quality evidence + uncertainty | `scene_observe_get` producer 完成后才可用 |
| `VisualEvidenceBundle@1` | multi-view AOV、metrics、failed gate、hash-only manifest | 不保存原图路径或截图作为版本真值 |
| `DesignCriticReport@1` | evidence-bound issue、metric、threshold、part/material target | Codex typed critic 或 deterministic critic 输出，必须引用 evidence hash |
| `RepairIntent@1` | bounded single-Part/MaterialZone repair proposal | 只能进入 prepare/recompile/readback/compare；不得直接写版本 |
| `ParametricDesignKitManifest@1` | Housing/Panel/Vent/Joint/Sensor/Frame 等 macro catalog | 每个 macro 展开为 typed Geometry/Appearance program，并有 validator/benchmark |

新增这些合同前必须更新 contract manifest、Schema checker、producer/consumer tests、MCP tool docs、Viewer docs 和 evidence；不能只创建空 Schema。

`GeometryPrepareResult@2` 是闭合的短生命周期 MCP 返回，只包含 candidate、job、operator catalog 与 `ArtifactReadback@2`；它不应额外暴露持久 evidence。`GeometryQualityReport@2` 只表示 strict hard gate 已通过的 quality CAS receipt，失败走 typed rejection 而不是伪造 `hard_gate_passed=false` 的该 Schema。`GeometryCandidateEvidence@1` 是 Runtime/Store 的 candidate-bound durable provenance：它绑定 program、artifact、readback、quality、catalog/readback-config 和可选 reference hash，并由 confirm/restore reread 使用。当前 source-focused PASS 不等于新的安装包、Desktop live、PBR、reference similarity、human review 或 360°证据。

## 3. 通用字段

每个持久/跨进程对象必须有：

```text
schema_version
id
project_id (适用时)
created_at
canonical_sha256
parent_refs / lineage
```

永久写请求增加 `base_version_id`、`prepared_object_id`、`prepared_object_sha256`、`approval_receipt_id`、`idempotency_key`。所有 ID opaque 且不能含用户名/路径。

## 4. 规范化与 hash

- UTF-8、明确 key 排序、数值/单位 canonicalization、禁止 NaN/Infinity/负零歧义；
- 时间使用 UTC RFC3339，hash 使用 SHA-256；
- 二进制只保存 CAS hash/MIME/size，不内联；
- 缺失与 `null` 语义不同，Schema 必须明确；
- unknown property 默认拒绝；
- enum 扩展按版本处理，不能静默映射；
- 任何 renderer/worker/platform 影响结果的配置进入输入 hash。

## 5. 几何和单位

默认米、右手坐标系、Y-up（最终 GLB lowering 明确转换）。每个长度/角度/颜色/纹理字段声明单位、范围和精度。Geometry Operator 只允许命名 typed 参数，不接受 JSON pointer、代码、URL 或路径。

## 6. 版本策略

同一 `@1` 只允许向后兼容的 optional 增加，且 validator/consumer 已知默认；破坏性变化创建 `@2` 和显式迁移。Runtime/MCP/Viewer/Worker 协商 contract set digest；不兼容时写路径关闭。

## 7. 负向 Gate

每个 Schema 至少测试 unknown fields、超长字符串/数组、深嵌套、非有限数、错误单位/ID/hash、路径/URL/secret-like 字段、循环 DAG、预算溢出、stale base、重复 key 和版本不兼容。
