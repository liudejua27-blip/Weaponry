# ForgeCAD Runtime Schema 规范

2026-08-22 `CandidateMaterialSurfaceQuality@1` public positive fixture：`Geometry → CandidateTopologyQuality@1 → AppearanceProgram@3 → TextureBuild@2 → SurfaceBake@1 → AppearanceSourceLineage@1 → CandidateMaterialSurfaceQuality@1` 的 `prepare → same-key replay → get → Runtime drop/reopen → restart get` 通过 **1/1（111.72s）**；Runtime focused **5/5**、Store full **74/74**、Contracts **350**。CAS inventory unchanged；stable `artifact_id` 与 GLB object SHA-256、MaterialPack CAS kind 精确区分，合法 UV/tangent rebuild 不计入 geometry-preservation 漂移。该结果仅为 `structural_only`；V2 animated-socket-particles 仍无完整 public `prepare → Store → restart get`，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/candidate-material-surface-quality-public-positive-source-gate-20260822.json`。

最终同 cohort 修订口径：强制 build cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` 下 material focused **5/5（112.63s）**；Runtime full **310 passed / 0 failed / 20 ignored**（330 total，201.91s），且 public material fixture 明确在该 full run 内执行。此前 **111.72s** 仅为 public fixture 单测时长；两者都只支持 `structural_only`，不提升 visual/commercial、human/engine 或 stage/confirm/version/export 状态。

数值口径：当前 manifest 为 **375 schemas**；本文的 291/290/284/271/264/257/231/229/227/221/215/210/204/201/197/195/193/191/187/177/175/173/170/168/166/164/162/160-schema 记录仅作 historical prior slice 保留。

2026-08-22 `FictionalEnergyVfxAnimatedSocketParticlesSequence@2` 双候选 source slice：Contracts **350**；Store V2 focused **2/2**、Store full **74/74**；Runtime V2 仅低层 focused **6/6**、cargo check **PASS**；MCP V2 **3/3**；同 cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` Runtime full **309 passed / 0 failed / 20 ignored**（191.06s）、MCP full **128 passed / 0 failed / 0 ignored**（1.93s），这些是全量回归，不是 V2 public `prepare → Store → restart get` 正向 fixture。V1/V2 隔离；V2 仅证明 1..16 frame、geometry/appearance 双 candidate/delivery/AnchorSet bridge 以及 Store FK/reachability/idempotence/conflict/rollback 的结构面。完整双候选 public Runtime `prepare → Store → restart get` 正向 fixture 尚不存在，durable end-to-end=`NOT_RUN` / `BLOCKED_FIXTURE_CHAIN`，不能声称正向 durable。该 slice 为 `structural_only`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/fictional-energy-vfx-animated-socket-particles-v2-dual-candidate-source-gate-20260822.json`。

2026-08-21 `GodotGameWeaponImportReceipt@1`：新增一个 closed 聚合合同，绑定 delivery manifest、static socket key 与 3 个 LOD derived hashes、animated socket key 与 derived hash、CollisionProxySet canonical sidecar hash、Godot binary/version/build、fixed harness hash 和 4 个 imported scene projections。真实 Godot `4.7.2.stable.official.ed1daf0bf` headless evidence 已通过：LOD triangles `304 > 176 > 112`，每 scene 5 meshes/相同 materials/6 non-rendering sockets parent/local TRS exact，10 glTF channels→2 semantic tracks、cross-loader t0/half-duration TRS exact、两个 named socket follow、5 个 `BoxShape3D` sidecar readback exact。static/animated 来自独立 Runtime source cohorts；没有 Runtime durable Godot link。commercial engine=false；Unity/Unreal、physics/hitbox、visual/human、confirm/export `NOT_RUN/false`，quality `structural_only`。当前合同总数 **291**，工具仍 **69 read + 49 write = 118**。

2026-08-20 `geometry-program-v2.schema.json` 的 closed `$defs` 新增 `energy_core_parameters`，并以 exact operator→parameter branch 绑定 `forgecad.geometry.energy-core@1`；component 限于 guard-ring、mechanical-ring、emitter-core、mechanical-backplate，schema 文件总数仍为 195。ActionRun/Critic/RepairIntent、Modifier Stack、PDK 和 fictional-energy-rifle profile 的 operator 枚举同步，禁止跨分支参数与未知可执行字段。

2026-08-20 Candidate-bound Viewer Provenance Graph 新增 `viewer-provenance-graph-request.schema.json` 与 `viewer-provenance-graph.schema.json`。它只读绑定 exact project/candidate/candidate-state/artifact，完整或失败地投影 Geometry evidence、Operator DAG、GLB、strict readback、geometry quality，以及可验证时的单个 visual/AOV 和 MechanicalAnimationClip 分支；固定 64 nodes / 128 edges / 1 MiB，不内嵌 GLB/PNG，不写 Runtime 状态。缺少独立 durable 历史的 Modifier Apply、Boolean Preview、Subdivision sidecar 与 DesignSession 必须作为 omitted/unknown 明示，结构证据不能提升视觉或 360°质量。该 slice 当时 manifest 为 195 schemas，MCP 工具为 54 read + 36 opt-in write = 90。

2026-08-19 candidate-bound Modifier Apply slice 新增 `geometry-modifier-apply-request.schema.json` 与 `geometry-modifier-apply-result.schema.json`。Request 只接受公开可读的 durable candidate/artifact/readback/program/catalog/config、一个 stable Part、闭合 modifier stack、current head/idempotency/input hash 与 1 MiB 边界；Result 绑定 source/new candidate、source/derived Worker cohort、program/artifact/readback/evidence、target terminal、preserved Part IDs、immutable CAS sidecar 与 `structural_only`。它不允许 Python/plugin/reference override，不 confirm/version/export，不代表视觉质量。该 slice 当时 manifest 为 191 schemas。

2026-08-19 Authoring Mesh Edit Prepare slice 新增 `authoring-mesh-edit-prepare-request.schema.json` 与 `authoring-mesh-edit-prepare.schema.json`。Request 封闭 project/source/current base、完整 preview、expected preview hash、幂等键、1 MiB 和 input hash；Result 封闭 source/new candidate、program/artifact/readback/evidence/Worker cohort/edit lineage、Job、审批锁、无 version/export、structural-only limitations 与 canonical hash。该 slice 当时 manifest 为 191 schemas。

2026-08-19 historical Authoring Topology/Edit Preview slice 新增 `authoring-topology-request.schema.json`、`authoring-topology.schema.json`、`authoring-mesh-edit-preview-request.schema.json` 与 `authoring-mesh-edit-preview.schema.json`。Topology request 只接受公开 `geometry_prepare` 可获得的 candidate/artifact/readback/program/catalog/config/node/Part/policy/budget 绑定；Runtime 内部从 durable evidence 派生并重验 GeometryProgram object/canonical。Topology 输出 exact source V/E/Loop/Face 与 node/Part lineage；Edit Preview 仅允许 sorted translate 或 boundary triangle/planar-convex-quad single-face extrude，并绑定 source/derived program/topology/replay hashes、no-write、structural-only 与 1 MiB。该 slice 当时 manifest 为 187 schemas，现行总数见本文顶部 195。

2026-08-19 historical Render Evidence Replay slice 新增 `render-evidence-replay-request.schema.json` 与 `render-evidence-replay.schema.json`。Request 封闭 candidate-state hash、完整 Integrity request 和固定 read-only replay policy；Result 封闭 exact candidate/artifact/camera/RenderSet/request/integrity hashes、artifact-only appearance 限制、同 cohort/profile 绑定、有序九 AOV source/first/repeat raw-byte 与 decoded-pixel hashes/sizes、no-write、1 MiB 与诚实 limitations。该 slice 当时 manifest 为 177 schemas，现行总数见本文顶部 195。

2026-08-19 historical manifest slice 新增 `mechanical-pose-geometry-preview-request.schema.json` 与 `mechanical-pose-geometry-preview.schema.json`。Request 封闭嵌套 single-tick pose request、固定 transient preview policy 与 outer input hash；Result 封闭 exact source lineage、application policy、per-Part rest/posed/delta、derived GeometryProgram、transient artifact strict readback、no-write、structural-only、limitations 与 canonical hash。该 slice 当时 manifest 为 175 schemas；现行总数为 195。

2026-08-19 historical manifest slice 新增 `subdivision-artifact-lineage-sidecar-request.schema.json`、`subdivision-artifact-lineage-sidecar.schema.json` 与 `subdivision-artifact-lineage-link.schema.json`。Request 封闭 prepare/get 共同输入；Sidecar 继承并收紧 exact artifact lineage payload，固定 immutable CAS/structural-only/no-cross-version；Link 绑定 request/candidate/artifact/readback/evidence/node/sidecar/lineage/artifact-binding hashes并嵌入完整 sidecar，无时间戳以保持幂等。该 slice 当时 manifest 为 173 schemas。

2026-08-19 historical manifest slice 新增 `subdivision-artifact-lineage-request.schema.json` 与 `subdivision-artifact-lineage-projection.schema.json`。Request 封闭 candidate/artifact/readback/node/budget/canonical；Projection 复用完整 root-lineage schema，并封闭 durable evidence/program/catalog/config hash、唯一 direct source primitive、full-GLB replay identity、primitive-local triangle ranges、no-write、not-persisted、structural-only 和 canonical。该 slice 当时 manifest 为 170 schemas。

2026-08-19 当前 manifest 新增 `subdivision-topology-lineage-request.schema.json` 与 `subdivision-topology-lineage.schema.json`。Request 封闭 exact `GeometryProgram@2`、target `subd-cage@2` node、1..25,000 element budget 和 canonical；Result 封闭完整作用域内的 control vertex/edge/quad root → evaluated quad topology 数组，固定 program/evaluation-bound ID、no artifact/readback/GLB binding、no corner/child-path/weight、no-write、`structural_only` 与 canonical/lineage hash。该 historical slice manifest 为 168 schemas。

2026-08-19 historical crease-aware manifest slice 新增 `subdivision-crease-evaluation-request.schema.json` 与 `subdivision-crease-evaluation-result.schema.json`。Request 是 closed、normalized-input-hash-bound 的 3..16 grid / 1..2 level / integer edge sharpness policy；Result 绑定 control-cage/crease/policy/topology/program/catalog/canonical hash并明确 `structural_only`、read-only projection 未编译、无 visual claim。`geometry-program-v2.schema.json` 另新增独立 `subd_cage_crease_parameters`，保持旧 `subd_cage_parameters` 不变。该 slice 当时为 166 schemas。

2026-08-19 historical Boolean Operand Lineage slice 当时 manifest 为 164 schemas。新增 closed `BooleanOperandLineageRequest@1` / `BooleanOperandLineage@1`：请求完整复用 `GeometryProgram@2` 并绑定 Boolean node、1..4096 run budget 与 canonical；结果固定两项 ordered operand、1..4096 连续 runs、lineage hash、no-write 和四项限制，Runtime 会从请求重算 operation/operand/source lineage，明确 evaluated face ID 不是原始 authoring face、跨 program 不稳定且未持久化到 GLB。receipt：`docs/evidence/mcp010f/blender-boolean-operand-lineage-source-gate-20260819.json`。

2026-08-19 historical Render Evidence Integrity slice 当时 manifest 为 162 schemas。新增 closed `RenderEvidenceIntegrityRequest@1` / `RenderEvidenceIntegrity@1`：request 绑定 13 项 exact scope/hash 与自身 canonical；result 分离 ArtifactReadback/RenderSet/comparison/quality 的 object/canonical hash、AOV `cas_object_sha256`/`bytes_sha256`、comparison mask、same-camera identity、RenderProfile 和 8 项 threshold gate lineage，固定 read-only/no-write/1 MiB 与历史 receipt 不修复限制。

2026-08-19 historical Mechanical Pose Sequence Preview slice 当时 manifest 为 160 schemas。新增 closed `MechanicalPoseSequencePreviewRequest@1` / `MechanicalPoseSequencePreview@1`：请求绑定 existing RestFrame/PoseAction draft 和 1..16 个 tick；Runtime 强制严格递增/唯一/时长范围、逐 sample semantic recomputation、完整 lineage-bound ordered sequence hash、1 MiB canonical response cap 与 `structural_only` / no-write；MCP 另对 summary + `structuredContent` 的整个 `tools/call` response 执行 1 MiB wire Gate。receipt：`docs/evidence/mcp010f/blender-mechanical-pose-sequence-preview-source-gate-20260819.json`。

2026-08-18 historical Parametric Group v2：该 slice 当时 manifest 为 158 schemas。新增 closed `ParametricDesignKitRequest@2` / `ParametricDesignKitProgram@2`，绑定 immutable template/catalog、parameters、instance、operator catalog、GeometryProgram、evaluation order、source map 与 canonical hash，并强制 `structural_only` / no-write。

2026-08-18 historical Mechanical pose：新增 `MechanicalRestFrame@1`、`MechanicalPoseAction@1`、`MechanicalPoseEvaluationRequest@1` 与 `MechanicalPoseEvaluationResult@1`，该 slice 当时 manifest 为 156 schemas。合同限定 64-link 无环机械层级、fixed/revolute/prismatic 单自由度关节、1000 Hz 整数 tick linear/clamp/rest action、candidate/artifact/readback/program/catalog/config 与 exact Part/source-node lineage；结果固定 `geometry_materialization=not-materialized`、`worker_evaluation=not-run-runtime-read-only-projection`。Schema 不声明 Armature、skin、IK/NLA/F-Curve、动画资产、package/live 或视觉 PASS。

2026-08-18 historical RenderProfile slice：新增 closed `RenderProfile@1`，该 slice 当时 manifest 为 152 schemas。它固定 ForgeCAD 自有 `forgecad-fixed-software@2` CPU raster backend、512×512、deterministic sampling、linear Rec709 D65 → fixed sRGB beauty transform、opaque film 与九个有序 AOV，并绑定 canonical/AOV/color-pipeline/ID-palette hashes；`RenderSet@2` 强制内嵌并复述这些 hashes。Schema 不声明 Blender/Cycles/EEVEE/OCIO/GPU/EXR/custom AOV、package/live 或视觉 PASS。前一 Subdivision v2 的 151-schema 状态保持历史 slice。

2026-08-18 historical Modifier evaluation v2 slice：新增 `GeometryModifierEvaluationRequest@2`、`GeometryModifierEvaluationSignature@1` 与 `GeometryModifierEvaluationResult@2`，该 slice 当时 manifest 为 149 schemas。Request 复用 closed Modifier Stack base/modifier 定义并只接受 null 或 canonical previous signature；source base operator/parameters 由 11 个 paired branches 绑定。Signature 绑定 ordered stage definition/input/output/cache chain，cache key 额外绑定 GeometryProgram canonical hash；Result 明示 `reuse_kind=semantic-signature-only` 与 `output_kind=geometry-program-canonical-sha256`。Schema 不声明真实 mesh cache、GLB readback、build cohort、视觉质量或用户批准。

2026-08-18 historical Bevel/Normal slice：`GeometryProgram@2` 的 closed operator/parameter set 与 `GeometryModifierStackRequest@1` 的 oneOf 已扩展 `bevel@1` / `normal-policy@1`，该 slice 当时 schema 总数为 146。Bevel 参数固定 direct source box、width/segments/profile/edge-scope/clamp；Normal 参数固定 corner-domain face-area × corner-angle、crease threshold、`keep_sharp=true`。Schema 接纳不单独构成执行 PASS；执行证据见 `docs/evidence/mcp010f/blender-bevel-normal-source-gate-20260818.json`。

2026-08-18 historical TopologySnapshot slice：新增 `TopologySnapshotRequest@1` 与 `TopologySnapshot@1`。请求是 closed、project/candidate/artifact/readback/program/catalog/config/policy 全绑定的单 Part readback；结果固定 `scope=part`、`complete=true`、`topology_space=evaluated-glb-triangle-mesh@1`、`id_scope=artifact-bound`、`cross_version_stable=false`、`quality_status=structural_only`，并保存有界 V/E/F/C、邻接、拓扑计数、lineage hash 与 corner normal/UV/tangent。该 slice 当时 manifest 为 146 schemas；下文 136/138/139/144 是更早 historical 数字。

2026-08-18 Modifier slice historical cohort：新增 `GeometryModifierStackRequest@1` 与 `GeometryModifierStackProgram@1`。前者是 closed、1..8 项、有序且 input-hash-bound 的只读 authoring request；v1 只允许 active unary transform/mirror/array。后者严格 `$ref` 完整 `GeometryProgram@2`，并返回 program/stack/canonical hash 与逐 stage effective evaluation hash，固定 `quality_status=structural_only`。它不表示已编译 mesh、candidate、视觉质量或用户批准。该 slice 当时 manifest 为 144 schemas；现行值以本文顶部的 195 为准。

版本：2026-08-13
2026-08-17 Reference Visual Structure：新增 `reference-visual-structure.schema.json`，作为 `SilhouetteTarget@1.visual_structure` 的可选嵌套合同。它保存重叠/共享视觉区域、连续曲面组、层级、深度证据和开放 line-flow；全局 silhouette 保持最高权威并禁止把视觉区域声明为功能部件。该 slice 当时 manifest 为 139 Schema；现行总数为 195 Schema。
2026-08-17 PDK v0：新增 `parametric-design-kit-request.schema.json` 与 `parametric-design-kit-program.schema.json`，由只读 `geometry_program_hash` 分支消费/生成。两者只描述 Runtime-owned typed macro 的输入、单节点 GeometryProgram 展开、Part/MaterialZone/parameter source map 与 structural-only 限制；不声明 candidate、视觉相似、PBR 或用户批准。
2026-08-17 历史 Stage 0 覆盖先后为 138 与 144 个 Schema；144-schema slice 当时为 41 read + 33 opt-in write = 74 tools。`fictional-energy-rifle-profile.schema.json` 与 `fictional-energy-rifle-plan.schema.json` 已进入 manifest，Profile 仍是 nonfunctional、source-only authoring aid；`repair-intent-run-request/result.schema.json` 已进入 manifest，结果保持 `confirm_allowed=false`，不能代替视觉质量或完整 orchestrator。当前机器真值为 195 schemas、54 read + 36 write = 90 tools。
状态：MCP005–MCP009 functional core 已落地；MCP006 历史 receipt 为 44 个 JSON Schema；MCP010B/C/D/E/F source 合同、Agentic Design Runtime contract family、weapon joint-multiview V2 contract family、fictional energy-rifle Profile/Plan、Modifier Stack、Modifier evaluation v2、TopologySnapshot、Subdivision evaluation v2、RenderProfile、Mechanical pose、Subdivision crease、Authoring Mesh Edit Prepare、typed particles、typed trails、静态 GLB sockets 与动画 GLB sockets contracts 已进入当前 manifest，当前源合同总数为 290。`RuntimeJobResult@1`、`repair-intent-run-request/result.schema.json`、Primary Form async Job 及 weapon coordinate/camera-rig/optimization 相关合同为长时间 Job、CAS-bound staged run 和多视图 promotion 的 hash-bound 边界。唯一 `in_progress` 为 `FGC-MCP010F`；历史 package/live receipt 仍按 cohort 单独保存。Agentic observe/plan 的真实 Runtime 嵌套只读 projection 已通过 producer/consumer conformance checker；durable session/checkpoint/RepairIntent prepare/readback、CAS-bound RepairIntentRun、窄范围 Primary Form action-run/readback 与本轮 joint source Gate 已各自有 source/runtime/MCP/隔离证据，但不等于 durable/reference/DesignSpec 完整 producer conformance、完整 Visual Evidence conformance、通用 orchestrator、Repair 应用或视觉 likeness。

Stage 0 机器真值为 `docs/evidence/mcp010f/current-benchmark-truth.json`；当前源码口径固定为 290 Schema、69 read + 49 opt-in write = 118，并绑定 290 个 Schema 文件内容集合哈希。attempt35 只是 provisional retained observation，为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`，packaged Viewer binding 为 `PASS_CURRENT_COHORT_BOUND_READ_MODEL`（不等于 attempt35 same-observation E2E）。Schema/producer 已实现不能补齐缺失 receipt 字段，也不能越过 PBR likeness、正式真人、export/restart 或 360 门。

`ReferenceCanvas@1` 的 view 项现可选绑定 `view_spec`、`target_sha256`、`mask_sha256` 与 `camera_claim.camera_canonical_sha256`；target/mask 必须成对出现，Runtime 还会检查它们与同一 `reference_id/reference_sha256`、CAS、相机和 evidence 的 lineage。`VisualEvidenceBundleProjection@1` 会投影这些 per-view hash，跨视图 compare 不得使用另一视图的 target，RepairIntent 的 evaluation kind 集合必须与 `coverage.supplied_views` 一一对应。旧 unbound 单视图仍显式使用 null，不能将兼容字段缺失解释为质量通过。

<!-- forgecad-stage0: schemas=402 schema_set_sha256=2f31c744134257e4d455cfc801a9be9e5c38ae81b1bc7fabd80d2002f968c4c7 read_tools=90 write_tools=69 total_tools=159 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->

## 1. 唯一来源

新合同源位于 `packages/forgecad-contracts/schemas/**`。MCP003 已验证首批 15 个 JSON Schema；MCP004 增加审批、候选、restore 和 diagnostic export records；MCP005 增加 reference admission/get records；`[transition-v1]` MCP006–009 已落地 `SubjectProfile@1`、`RepresentationPlan@1`、`AssemblyGraph@1`、`GeometryProgram@1`、`AppearanceProgram@1`、`RecipePlan@1`、`ArtifactReadback@1`、`RenderSet@1`、`QualityReport@1`、`ChangePrepareResult@1`、GLB export profiles 和 Skill manifest/list/get/receipt/eval records，共 44 个历史 JSON Schema。MCP010B/C/D/E/F、Agentic contract family 与 weapon joint-multiview V2 family 继续增加当前 V2/evidence/target/camera/Rig/fit/Part/candidate compare/session/checkpoint/RepairIntent/action-run/Job result/RepairIntentRun/Modifier Stack/TopologySnapshot/Subdivision evaluation/RenderProfile/Mechanical pose/Subdivision crease Schema，当前 manifest 共 177；`RenderSet@2` 另绑定 Render Worker cohort/status 与完整 RenderProfile lineage。这不改写 44-contract 历史 receipt。全部 Schema 均须可解析、带 draft/id、contract manifest 为 `forgecad-runtime-contracts@1` 且声明 `model_calls=false`，manifest 与目录无漂移。Rust records 由 `forgecad-contracts` 维护；完整生成器、TypeScript 生成和额外 transport/未来宿主 conformance 仍未完成。旧 Concept/Weapon/Provider/Agent Schema 已删除。

## 2. 首批 Schema

### Runtime/MCP

`RuntimeCapabilities@1`、`RuntimeTool@1`、`RuntimeProject@1`、`RuntimeSnapshot@1`、`RuntimeJob@1`、`RuntimeJobResult@1`、`RuntimeError@1`、`RuntimeJobEvent@1`、`RuntimeResourceContents@1`、`Selection@1` 已落地；MCP annotations/resources Schema snapshot 在 `docs/evidence/mcp003/`。

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

当前 high-quality contract path 固定为 `GeometryProgram@2` detail → `ArtifactReadback@2` strict readback → `AppearanceProgram@2`（受前序门控制）→ `RenderSet@2` 九 AOV + Render Worker cohort binding → `ReferenceComparisonReport@1` strict compare → `VisualReviewReport@1` / `QualityReport@2`。`[transition-v1]` `GeometryProgram@1` primitive-only、`AppearanceProgram@1` 与 `RenderSet@1` 四 pass 只属于历史兼容，不得提升为当前 high-quality contract path。

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
