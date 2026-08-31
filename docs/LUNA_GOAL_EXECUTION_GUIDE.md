# Luna Goal 执行指南：ForgeCAD 单用户 MVP

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

> 2026-08-28 Goal 当前完成 `FPS-FORM-04BE-L`。用户授权 `receiver-upper` 后，Codex/Runtime 已执行 4 个 product-owned 单边回撤候选；全部结构通过、视觉拒绝，父候选保留。下一原子固定为 `FPS-FORM-04BE-M`：只读分析 right target 的 depth winner/Part-ID/occlusion；不得继续盲目放大回撤或同时修改其他 Part。

> 2026-08-28 Goal 当前完成 `FPS-FORM-04BE-J/K`。Luna Max 并行拓扑、证据和文档审计已确认：真孔拓扑存在，但 approved left target 的剩余遮挡尚未被精确归因；8 个候选全部拒绝，父候选保留。下一原子固定为 `FPS-FORM-04BE-L`：只读绑定 exact GLB/camera/mask/depth/Part-ID，输出目标区域逐 Part depth-winner 与遮挡排名；不得继续盲扫孔参数，不得提前修改 `receiver-upper` 或放宽六视图门。

> 2026-08-28 Goal 当前完成 `FPS-FORM-04BE-I`。4 个 `side-panel-a` 20/40mm min/max-X 试验全部因六视图回退且 left trigger void 仍 sealed 被拒绝，母候选已确定保留。Luna/Codex 下一原子固定为 `FPS-FORM-04BE-J`：重设计且注册能在二维孔洞内产生非零可见响应的 `side-panel-a` aperture/边界形变族；不得跳到 `receiver-upper`、多 Part 同改或放宽六视图门。

> 2026-08-28 Goal 当前完成 `FPS-FORM-04BE-H`。Luna/Codex 下一原子固定为 `FPS-FORM-04BE-I`：只注册并执行 plan canonical=`fe7c8ecf…680b61` 的 step 1 `side-panel-a` 四个试验，从 20/40mm min/max-X 回撤中仅保留目标孔改善且六视图不回退的结果。未达门时不得进入 `receiver-upper`，不得自行扩大 Part 范围。

> 2026-08-28 Goal 当前完成 `FPS-FORM-04BE-G`。Luna/Codex 下一原子固定为 `FPS-FORM-04BE-H`：以 canonical=`3d3cd762…e7196` 为唯一输入，编制同时绑定 left `side-panel-a` 与 right `receiver-upper` 的双视图 aperture repair plan；不得继续修改 rear-stock，不得执行 geometry、创建候选/FormQualityV2/secondary/confirm/version/export 或进入 High→Low→UV→Bake。

> 2026-08-28 Goal 当前完成 `FPS-FORM-04BE-D`：Luna/Codex 只能把 canonical=`d6f74060…85fd` 的 registered typed plan 作为下一原子输入，不能自行改 camera/reference/base 或注入 mesh/vertices。下一步执行后必须重跑同 baseline/批准六相机 54 AOV、strict owner-void、negative-space/line-flow 与 fresh FormQualityV2；失败即继续 `QUALITY_TARGET_NOT_MET`，不得自动进入 High→Low→UV→Bake。

> 2026-08-28 `FPS-FORM-04BE-C` 并行约束：Luna 可用于合同、Runtime/Store seam、证据与文档只读审计，但 Runtime 仍是唯一写者，主 Agent 仍是唯一 Gate reviewer。当前 durable 54-AOV sidecar/restart PASS 不改变 `QUALITY_TARGET_NOT_MET`；子智能体不得抹平 left boundary 回退、rear-three-quarter owner pixels=0 或提前创建 FormQualityV2/secondary/High。下一原子 `FPS-FORM-04BE-D` 只允许从 exact CrossView/FormArt hashes 派生 bounded typed repair plan，不改变相机、参考、original baseline 或 current base。

> 2026-08-27 `04AK` 并行约束：Luna 只围绕同一 D1 纵向链做 source audit/compile acceleration。fresh-baseline preflight 已完成但 materializer 不可用；不得由子智能体绕过用户 orientation authority、复用历史 FormArt 或提前启动 High/Low/UV/Bake。下一并行边界只允许 Runtime-owned producer、失败补偿审计和文档/证据核对。

> 2026-08-27 `04AJ` 并行约束：Luna 审计只用于收紧同一真实 D1 纵向闭环。620/680 已有 zero-write proposal，但不得让子智能体绕过用户方向批准或跨 cohort 复用历史 FormArt；固定顺序是 user authority → camera lineage/restart → same-cohort baseline refresh → 单一 notch → six-view gates。700/700 的 `-3081ppm / REJECTED_REGRESSION` 必须继续保留，不能因更小参数而推定成功。

> 2026-08-26 Goal 执行同步：Luna 并行切片已交付 AuthoringMesh V2 `FaceExtrude` kernel、Native High original-topology bridge 与真实 D1 binding 设计审计；主执行器已完成 rear-stock source-bound genesis/restart。当前公共面 **527 schemas / 114 read + 86 write = 200 tools**。下一并行任务不得扩张文档或 fixture，固定围绕 `MoveVertices`、revision lower/proposal 与六视图门；Stage/quality 不因 source compile 晋级。

> 2026-08-26 `04AF` 交接约束：子智能体研究或 source compile 必须服务于同一真实资产纵向闭环。已被六视图拒绝的 rear-stock 方案不得包装成进展；`AuthoringMesh@2` 的下一任务是绑定真实 D1 并补齐美术编辑操作，而不是再复制一份平行合同。

> 2026-08-26 04AE 现行 source：**525 schemas / 112 read + 84 write = 196 tools**。并行 Luna 结果只允许记 source compile；执行 real D1 CameraLock child 前必须保留 orientation-specific user approval，不得把诊断旋转写成用户回执。

> Luna 商业武器任务必须领取纵向原子，不得并行写同一 Runtime/Geometry/Stage 真值。研究可并行，落地顺序保持 CameraLock/Form → AuthoringMesh → one-Part High/Low/UV/Bake → Material/FPS/Engine/Human。详见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 `FPS-FORM-04AD` 权威增量：当前合同面为 **518 schemas / 111 read + 83 opt-in write = 194 tools**。新增 `ProductionWeaponSemanticLandmarkOrdering@1` 只表达 Runtime-derived 的 3D source/subject-axis 顺序，明确 `target_landmark_arrays_present=false / metrics=NOT_PRESENT`，不得冒充 2D landmark；`ProductionWeaponAuthoredViewOrientation@1` 将诊断变换与用户方向回执分开；`RegisteredCameraRigCalibration@2` 只有绑定 promotable authored rear3q receipt 才能物化。定向 Contracts/Runtime/MCP compile 与 518-schema checker PASS。真实 D1 尚无 orientation-specific user receipt，因此保持 `BLOCKED_AUTHORED_REAR_THREE_QUARTER_ORIENTATION`、Stage=`camera-calibrated`、secondary=`NOT_CREATED`、quality=`QUALITY_TARGET_NOT_MET`，不 confirm/version/export。旧 `@1` 保持历史真值；durable 落点采用 CameraLock 的 additive child lineage，不复制/自动升级整张旧记录。

> 2026-08-26 商业 Goal 执行：多 Agent 只允许并行做只读研究、互不重叠的代码模块或审计；Runtime/Geometry/Render 仍由一个主写者和一个主 Gate 汇总。同一时刻只推进一个真实 Hero candidate、一个最高影响缺陷和一个 Stage；每个模块先 compile 再尽快取 `PASS_ASSET`，禁止以工具数或重复测试填充进度。

> 2026-08-26 最新权威 source 口径（取代下方 2026-08-25 的“最新/当前”计数）：**518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**。Low quad draft 的 current provenance 为 candidate-bound，仍为 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；其 prepare→同键重放→Runtime drop/reopen→get 在隔离 current-cohort fixture **1/1 PASS**。Hero UV 已有 **7 个 registered contracts**，并已接入 Store/Runtime/MCP public `hero_uv_durable_get/prepare`；其 prepare→同键重放→Runtime drop/reopen→get **1/1 PASS**，四个 Hero CAS roots 已纳入 linked/GC 判定。该 slice 仅为 `structural_only`，不是 artist unwrap、visual、human、engine、commercial 或 packaged PASS；proposal=`registered=false`。当前不推进 Stage、confirm、version 或 export；Stage=`camera-calibrated`、`secondary-form-approved=NOT_CREATED`、`FPS-HIGH-05=NOT_PASSED`、visual=`QUALITY_TARGET_NOT_MET`、HQ360=`BLOCKED_REFERENCE_COVERAGE`。证据：`docs/evidence/mcp010f/commercial-weapon-hero-uv-durable-restart-source-gate-20260826.json`。

> 2026-08-25 历史快照（已由上方 2026-08-26 权威口径取代）— source 基线：**499 schemas / 107 read + 79 opt-in write = 186 MCP tools**；Native High=`PASS_SOURCE_DURABLE_RESTART_MCP`，Low quad draft/Hero UV=`PASS_WORKER_SOURCE_ONLY`，Viewer Art Director matrix=`PASS_SOURCE_BUILD`。Luna 不得把这些源码/持久化数字写成 packaged、candidate visual、human、engine 或 distribution PASS。

2026-08-25 commercial weapon research rule：涉及商业级武器质量时，先读 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`，再领取当前唯一原子任务。不得把视频/教程/GitHub 研究、工具数量、2K 贴图或漂亮渲染写成实施完成；必须逐项核对 Form、AuthoringMesh、High、Low、UV、Cage/Bake、Material、FPS、Engine、Human 的结构/视觉/真人/引擎/分发状态。研究 lane 默认 read-only，只有主 Agent 可在 Runtime 单写者边界和正式 Gate 下整合。

2026-08-22 `CandidateMaterialSurfaceQuality@1` public positive fixture：`Geometry → CandidateTopologyQuality@1 → AppearanceProgram@3 → TextureBuild@2 → SurfaceBake@1 → AppearanceSourceLineage@1 → CandidateMaterialSurfaceQuality@1` 的 `prepare → same-key replay → get → Runtime drop/reopen → restart get` 通过 **1/1（111.72s）**；Runtime focused **5/5**、Store full **74/74**、Contracts **350**。CAS inventory unchanged；stable `artifact_id` 与 GLB object SHA-256、MaterialPack CAS kind 精确区分，合法 UV/tangent rebuild 不计入 geometry-preservation 漂移。该结果仅为 `structural_only`；V2 animated-socket-particles 仍无完整 public `prepare → Store → restart get`，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/candidate-material-surface-quality-public-positive-source-gate-20260822.json`。

最终同 cohort 修订口径：强制 build cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` 下 material focused **5/5（112.63s）**；Runtime full **310 passed / 0 failed / 20 ignored**（330 total，201.91s），且 public material fixture 明确在该 full run 内执行。此前 **111.72s** 仅为 public fixture 单测时长；两者都只支持 `structural_only`，不提升 visual/commercial、human/engine 或 stage/confirm/version/export 状态。

数值口径：当前 source 为 **518 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**；Low provenance 为 candidate-bound，Hero UV public `hero_uv_durable_get/prepare` 与四个 CAS roots 的 linked/GC 仅为 `structural_only`，不是 artist unwrap、visual、human、engine、commercial 或 packaged PASS。

2026-08-25 Native High/GLB/durable 边界：`HighMeshArtifact@1` 与 `NativeHighDurable*` 是 ForgeCAD-owned bounded source slice，不是 GitHub adoption、active Skill 或视觉结论。Runtime durable prepare/get 在 exact durable AuthoringMesh binding 后双回放 High/GLB、校验 cohort/bytes/hash，并把 derived artifact/link 写入 CAS/Store；同 cohort restart fixture 与公共 MCP source-focused receipt 已通过。全链路保持 `source_only`/`structural_only`、no Stage/confirm/version/export；packaged/visual/human/engine 仍不得记 PASS，proposal 继续 `registered=false`。

商业级 Form、AuthoringMesh、High、Low/Retopo、Hero UV、Cage/Bake、Material/AssetPack、FPS/LOD、engine readback、独立人评和 export/restart 必须由 ForgeCAD typed Schema、固定 Worker、Runtime 单写者、strict readback 与正式 gates 内建。Blender、Substance Designer/Painter、其他 DCC、GitHub 脚本/插件、模型权重和远程 image-to-3D 只能作为研究参考，不能成为 Runtime/CAS/Stage 真值。

2026-08-22 `FictionalEnergyVfxAnimatedSocketParticlesSequence@2` 双候选 source slice：Contracts **350**；Store V2 focused **2/2**、Store full **74/74**；Runtime V2 仅低层 focused **6/6**、cargo check **PASS**；MCP V2 **3/3**；同 cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` Runtime full **309 passed / 0 failed / 20 ignored**（191.06s）、MCP full **128 passed / 0 failed / 0 ignored**（1.93s），这些是全量回归，不是 V2 public `prepare → Store → restart get` 正向 fixture。V1/V2 隔离；V2 仅证明 1..16 frame、geometry/appearance 双 candidate/delivery/AnchorSet bridge 以及 Store FK/reachability/idempotence/conflict/rollback 的结构面。完整双候选 public Runtime `prepare → Store → restart get` 正向 fixture 尚不存在，durable end-to-end=`NOT_RUN` / `BLOCKED_FIXTURE_CHAIN`，不能声称正向 durable。该 slice 为 `structural_only`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/fictional-energy-vfx-animated-socket-particles-v2-dual-candidate-source-gate-20260822.json`。

2026-08-20 `energy-core@1` 验收必须同时核对：四种 closed component 与全入口 exact binding；inner/outer radius、solid exact-zero、depth/segments 和预算负门；Part/source/material 精确映射、boundary/non-manifold/winding、strict GLB/readback/lineage；合法参数差分与 determinism；Runtime/MCP Agentic typed patch 和 Skill trust。不得把同心轴组件语法写成通用材质图或视觉质量 PASS。

2026-08-19 Authoring Mesh Edit Prepare 验收必须同时核对：write opt-in/requiresConfirmation；expected preview 由 Runtime exact 重算；current head 与 source scope；actual fixed Worker same-cohort；strict GLB/readback/quality/evidence；candidate/Job/event/audit/idempotency 同事务；失败临时 CAS 清理；重复请求精确回放；成功仍无 confirm/version/export。不得写成 Blender BMesh/Python/plugin 或视觉 PASS。

2026-08-19 Render Evidence Replay 验收必须同时核对：exact integrity/candidate state；current GLB strict readback 与 persisted `ArtifactReadback@2`；actual fixed Geometry/Render Worker 同 cohort；source/first/repeat 九 AOV 原始 PNG 和 decoded RGBA8 pixel exact；profile/AOV order/color semantics；Runtime 重启后可重建；candidate/version/CAS/SQLite 无写入；MCP 整响应 1 MiB。不得把单 cohort repeat 写成跨平台 determinism、视觉质量或 Cycles/EEVEE parity。

2026-08-19 Mechanical Pose Geometry Preview 验收必须核对：exact candidate/artifact/readback/program/catalog/config/rest/action binding；`D = PoseWorld × inverse(RestWorld)`；每 Part final output 的唯一纯 source ownership；derived program 新 hash、fixed Worker compile、strict GLB readback；Euler 数值等价与 gimbal fail-closed；重复请求确定性和 CAS/SQLite/candidate/version 零写入。不得把 caller-authored rest frame 写成原资产 rig/pivot provenance，也不得写成 Blender Armature/skin/animation 或视觉 PASS。

2026-08-19 Subdivision artifact-lineage sidecar 验收必须同时核对：只有显式 prepare 可写；prepare 先通过 reconstructed exact-artifact replay；CAS sidecar 与 SQLite link 固定 request/candidate/artifact/readback/evidence/node hashes；重复请求幂等，漂移 fail closed；getter 重启可读且调用前后 candidate/version 不变；CAS 篡改与完整 MCP wire 1 MiB 拒绝。不得把 durable candidate-local sidecar扩写为跨版本 V/E/F/C identity 或视觉 PASS。

2026-08-19 Subdivision artifact-lineage 验收必须同时核对：durable V2 evidence project/candidate/artifact/program/readback/catalog/config exact binding、strict GLB readback、persisted program full-GLB byte-for-byte replay、唯一 direct `subd-cage@2` primitive、root quad ranges与 primitive-local triangle order、Runtime restart readback、1 MiB/25k、getter no-write 和 rehashed semantic forgery。不得把 reconstructed projection 写成持久 CAS sidecar、glTF vertex/edge/corner identity、跨版本 ID 或视觉 PASS。

2026-08-19 Subdivision root-lineage 验收必须核对 actual fixed Worker evaluator、Runtime 独立计数/coverage/range/crease-chain 重验、3×3 level2=442、16×16 level2=22,802、25,000-element budget、完整 MCP wire 1 MiB、no-write 与 semantic-forgery negatives。`complete=true` 只限 declared control-root→final evaluated quad topology，不得扩写为全 V/E/F/C、逐级 child path、corner/weight lineage、artifact/GLB identity 或视觉 PASS。

2026-08-19 crease-aware Subdivision 验收必须分别检查：read-only authoring projection 不写状态；`geometry_prepare` 才进行 Worker compile/CAS candidate；smooth/dart/crease/corner/decay golden 和 strict GLB readback；request-bound result validator、closed schema/1 MiB 与负向预算；GPL/OpenSubdiv reference-only。不得把当前 bounded @2 扩写为 Blender/OpenSubdiv parity、Modifier Stack crease、视觉或 package PASS。

2026-08-19 historical Boolean Operand Lineage source truth 当时为 164 schemas、19/19 active operators、45 read + 33 opt-in write = 78 tools。Luna 验收 `boolean_operand_lineage_preview` 时必须核对 exact program/catalog/node、Runtime 独立重算 operation/左右输入/source lineage、连续 run 覆盖、operand 计数、lineage/canonical hash、完整 MCP wire 1 MiB 和 no-write；必须明确 evaluated-face ID 不是原始 authoring face，不跨 program 稳定，也不持久化到当前 GLB。receipt：`docs/evidence/mcp010f/blender-boolean-operand-lineage-source-gate-20260819.json`。

2026-08-19 historical Render Evidence Integrity source slice 当时为 162 schemas、19/19 active operators、44 read + 33 opt-in write = 77 tools。Luna 对 Render Evidence Integrity 的验收必须分别核对 object/canonical/bytes hash、exact candidate/reference/camera lineage、九 AOV/mask 解码、threshold policy 与 no-write；只能写 `same_camera_verified`，不得推断 camera fit provenance、视觉通过、历史 receipt 修复或 Cycles/EEVEE parity。

2026-08-19 historical Mechanical Pose Sequence Preview source slice 当时为 160 schemas、19/19 active operators、43 read + 33 opt-in write = 76 tools。Mechanical Pose Sequence Preview 仅能经默认只读 `mechanical_pose_evaluate` 对同一 candidate-bound RestFrame/PoseAction 采样最多 16 个严格递增 tick；Luna 不得把它扩写为 Armature、skin、timeline、NLA/F-Curve、Python/plugin 生态或视觉通过。source receipt：`docs/evidence/mcp010f/blender-mechanical-pose-sequence-preview-source-gate-20260819.json`。

2026-08-18 historical Parametric Group v2 source：该 slice 当时为 158 schemas；仅能经默认只读 `geometry_program_hash` 调用三个 fixed first-party template，不能扩写为 Blender Geometry Nodes 或插件运行时。

2026-08-18 historical source slice 当时为 160 Schema、43 read + 33 opt-in write = 76 tools。`mechanical_pose_evaluate`、`topology_snapshot_get` 与 Subdivision v2 是只读结构投影；`RenderProfile@1`/AOV lineage 只能由 Worker/Runtime exact hash binding 生产。Luna 必须逐项回传 candidate/artifact/readback/Part/source-node/hash bindings，不得把 Mechanical pose 写成 Armature/skin/动画资产，不得把 evaluated topology 写成 BMesh/authoring topology，也不得把 structural PASS 晋升成视觉 PASS。

版本：2026-08-17
状态：Luna 强制执行协议；MVP host golden path 已收口；MCP010 质量轨道已批准。最新源码已安装为 Dev.app cohort `ce45110e3a5e6eaa5b5283e61f430e2338c7f06a2d09f4e75d4a21cb924f6a86`，package/probe PASS；相机/观察绑定修复的隔离真实参考回归仍以 cohort `18c9fb86cafc7e7baf2356e2efe9db404e6530fdeae54d33c0a9beba94fbae40` receipt 为准，已完成至 evaluate 后按 `QUALITY_TARGET_NOT_MET` blocked；无 confirm/version/export，live Desktop restart 仍 `NOT_RUN`。
当前任务：`FGC-MCP010A done`；`FGC-MCP010B blocked/deferred（Darwin OS memory hard cap NOT_RUN）`；`FGC-MCP010C done（source-focused PASS_WITH_UNRUN_VISUAL_GATES）`；`FGC-MCP010D/E source + packaged structural PASS（Manifold/xatlas/Validator/视觉子门 NOT_RUN）`；唯一 `in_progress` 为 `FGC-MCP010F`（Viewer source、packaged CLI read-model、原生窗口与核心控件 smoke PASS；provisional observation package binding/正式 VoiceOver/人评/360 `NOT_RUN/BLOCKED`）

## 1. Goal 目标

Luna 是仓库开发执行者，不是 ForgeCAD 运行时 Agent、Provider 或状态真值。当前 Goal 的代码主线和真实 Codex CLI 已完成一条用户授权图片 → typed 3D → PBR/fixed render → quality → approval/version → CAS GLB receipt 的 MVP host golden path；MCP010C source 已完成固定 renderer、九 AOV、reference compare、Codex typed visual review 以及 human review 合同/工具接口，MCP010D/E source 已完成真实硬表面 Operator、离线 AssetPack、UV atlas、MikkTSpace 和嵌入式 PBR，MCP010F source 已完成只读 Viewer 的 AOV/对比/部件筛选/材质区筛选/爆炸图/热图辅助。真实用户 likeness、同一 candidate 的 packaged Viewer、独立真人评分、PBR likeness、xatlas/Validator、export/restart hash 和 360 仍按独立证据推进，不继续堆复杂后台治理。

Stage 0 唯一机器可读真值为 `docs/evidence/mcp010f/current-benchmark-truth.json`：attempt35 只是 provisional retained observation，不是合格 benchmark；它为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`；packaged Viewer 来自不同 cohort/artifact，未绑定该 candidate。任何 Luna 状态更新都必须保留这些失败/未运行边界。


<!-- forgecad-reference-source: input=ENV_AUTHORIZED_PNG original_sha256=1964704a62ed7a841b4d49c370b8d46f4626e201daad29092a9c39a40b4c4109 intake=PASS_SOURCE_SIX_REFERENCE_EVIDENCE_CAS views=6 worker=PASS_SAME_COHORT_SIX_FIXED_VIEWS target=USER_REFINED_USER_CONFIRMED_REVIEWED_STRUCTURE user_confirmed_crop=PASS_USER_CONFIRMED_SEVEN_CROPS contour=PASS_USER_CONFIRMED_SIX_IDENTITY_CONTOURS negative_space=BOUNDING_REGIONS_CONFIRMED_EXACT_SUBTRACT_UNKNOWN line_flow=EXPECTED_ROWS_DURABLE_MATCH_NOT_PROVEN camera_lock_fixture=PASS_REAL_DURABLE_REPLAY_RESTART form_art_fixture=PASS_REAL_DURABLE_NOT_PROVEN form_quality_v2_fixture=BLOCKED_ZERO_WRITE_MISSING_LEGACY_CROSS_VIEW secondary_form_approved=NOT_CREATED fixture=PASS_REAL_1_OF_1_108.07S -->

不要把 Goal 写成“完善整个软件”后无边界并行修改。一次只领取一个 `FGC-MCPxxx`，先完成退出 Gate，再进入下一项。MCP005–009 是已完成的 functional core；MCP010A–F 严格串行，MCP011–013 保留可靠性、分发和正式发布职责。

## 2. 每次启动完整阅读

1. `/Users/liuchongjiang/Documents/武神/AGENTS.md`
2. `docs/DOCUMENTATION_MAP.md`
3. `docs/DOCUMENTATION_STATUS.md`
4. `docs/CODEX_HANDOFF.md`
5. `docs/ADR/0025-codex-only-mcp-3d-runtime.md`
6. `docs/ADR/0026-agentic-design-runtime.md`
7. `docs/FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN.md`
8. `docs/ARCHITECTURE_MODULE_BOUNDARY.md`
9. `docs/DEPRECATED_ISOLATION_PLAN.md`
10. `docs/RESET_MIGRATION_PLAN.md`
11. `docs/CODEX_EXECUTION_PLAN.md`
12. `docs/CODEX_TASK_INDEX.md`
13. `docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`
14. `docs/AUTHORITATIVE_STATE.md`
15. `docs/MVP_DELIVERY_PLAN.md`
16. `docs/MVP_TOOL_CATALOG.md`
17. `docs/EXTERNAL_PROJECT_ADOPTION.md`
18. `docs/LUNA_GITHUB_REPLICATION_PLAYBOOK.md`（处理 GitHub 研究或选择性源文件复刻时必读）
19. 本文件
20. 当前任务对应的 MCP/Schema/Compiler/Viewer/Skill/Test/Packaging 合同。

若冲突，按 `DOCUMENTATION_MAP.md` 解决；没有明确权威时先修文档，不自行混合两套架构。

ADR-0026、Agentic plan、模块边界和废弃隔离计划都是当前权威阅读链的一部分。当前已有 Agentic contract family、4 个 read-only projection tool、5 个 durable session/checkpoint prepare/readback tool 和隔离 Runtime/MCP/Viewer evidence；真实 Runtime 的嵌套只读 projection producer/consumer conformance 另已通过 `scripts/check_agentic_projection_receipt.py` 校验。Luna 必须把它们分别标为 `source/read-only projection PASS`、`nested projection conformance PASS` 与 `durable prepare/readback PASS`，不能写成 durable/reference/DesignSpec 完整 schema conformance、单动作 orchestrator、Repair execution 或高质量 PASS。

## 3. Goal 建议文本

用户已批准并显式继续以下 Goal；010A–E 的已完成/source 状态按账本保留，当前只执行唯一 `in_progress` 的 FGC-MCP010F：

```text
按照 AGENTS.md、docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md、docs/CODEX_TASK_INDEX.md 和本指南，保护 dirty worktree，一次只执行一个原子任务。当前源码固定为 **515 schemas、28 operator entries、111 read + 83 opt-in write = 194 MCP tools**，唯一 `in_progress` 是 FGC-MCP010F。Low provenance 为 candidate-bound，Low 保持 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`；Hero UV 的 7 个 registered contracts 已接入 Store/Runtime/MCP public `hero_uv_durable_get/prepare`，四个 Hero CAS roots 已纳入 linked/GC，durable prepare/replay/drop/reopen/get **1/1 PASS**。这些 slice 不是 artist unwrap、visual、human、engine、commercial 或 packaged PASS；Stage=`camera-calibrated`、`secondary-form-approved=NOT_CREATED`、`FPS-HIGH-05=NOT_PASSED`、`QUALITY_TARGET_NOT_MET`、HQ360=`BLOCKED_REFERENCE_COVERAGE`、proposal=`registered=false`，无 confirm/version/export。Agentic 设计调用必须先 `skill_get(ponytail-preflight@0.1.0)`；观察路径为 `scene_observe_get/design_stage_plan_get`，durable 路径为 `session_create_or_resume → checkpoint_prepare → checkpoint_get/session_get`，Repair 路径另需 `repair_intent_run_prepare` 的 CAS-bound staged run，恢复只允许 `checkpoint_restore_prepare` 生成 CAS-only RepairIntent，不能绕过 approval 或直接改 candidate/version。高质量路径只允许 `GeometryProgram@2` detail → strict readback → 九 AOV strict compare → typed visual review；`[transition-v1]` primitive-only 仅为历史兼容。Viewer source 和 packaged read-model/window/core-control smoke 已通过，但 attempt35 仍为 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，fit/compare camera `MISMATCH`，packaged Viewer 未绑定 provisional observation。正式 VoiceOver、真人、PBR likeness、export/restart hash 和 360 仍独立记录。禁止旧 Provider、付费 API、远程 image-to-3D、任意 Python/BlenderMCP、手工 GLB、heartbeat 或插件市场。
```

若用户要求大调整或架构重规划，应追加：

```text
同时遵循 ADR-0026、FORGECAD_AGENTIC_DESIGN_RUNTIME_PLAN、ARCHITECTURE_MODULE_BOUNDARY 和 DEPRECATED_ISOLATION_PLAN。目标是让架构和模块边界清晰：先隔离 superseded 文档/代码/模块，再按 ReferenceCanvas → DesignSpec → SemanticSceneGraph → stage gates → Visual Evidence → Critic/Repair 拆分后续任务。不得在当前脏工作树直接删除未知文件；废弃项先进入 archive/quarantine 并保持可恢复证据。
```

用户已经授权的 GitHub 研究必须追加：

```text
仅按 LUNA_GITHUB_REPLICATION_PLAYBOOK.md 研究 build123d、BlenderMCP、CadQuery、Manifold、MaterialX 的冻结 revision。每个项目先写 research-authorized receipt，选择性文件只进隔离缓存，随后做许可证/依赖/动态代码/网络/文件系统审查。默认输出是 ForgeCAD 自有 Schema 和 Rust rewrite；不运行 Python、Blender addon、socket、上游安装脚本或模型下载。未取得 approval: accepted、SBOM、恶意输入、确定性、资源和平台证据前，不改 lockfile、安装包、Runtime allowlist 或 active Skill。
```

真实 host 证据按下面的顺序运行；`<AUTHORIZED_REFERENCE>` 必须是用户明确授权的本地 PNG/JPEG，命令输出不得写入 Git、日志或 receipt：

```bash
cd /Users/liuchongjiang/Documents/武神
export FORGECAD_MCP005_REFERENCE="<AUTHORIZED_REFERENCE>"
export FORGECAD_MCP007_REFERENCE="$FORGECAD_MCP005_REFERENCE"
export FORGECAD_MCP007_CODEX_E2E=1
export FORGECAD_MCP009_REFERENCE="$FORGECAD_MCP005_REFERENCE"
export FORGECAD_MCP009_CODEX_E2E=1
npm run release:mvp
script/test_mcp005.sh
script/test_mcp007.sh
script/test_mcp009.sh
```

MCP005/007/009 的真实 Codex receipts 已为 `PASS`；复核时必须严格观察
`project_create → reference_import → geometry_prepare → artifact_readback_get → appearance_prepare → artifact_readback_get → quality_get → candidate_confirm → version_list → export_prepare → export_confirm → version_list`。Codex 的缺参重试、额外调用、OAuth 无工具、超时或 Desktop attachment 不可验证时，保留 `BLOCKED/NOT_RUN`，并把 observed prefix、Runtime/host 错误和下一条安全动作写入对应 evidence；不要放宽 probe 来制造完整闭环 PASS。

## 4. 任务领取协议

修改文件前记录：

```text
Task ID:
Dependency status:
Base commit / branch:
git status -sb:
git diff --check:
Existing dirty files in owned paths:
Owned paths:
Forbidden paths:
Current capability:
Target capability:
Baseline commands and exit codes:
Exit gates:
External dependency decisions:
Destructive actions / user approval:
```

若任务索引没有 `ready`，不得自行跳任务。领取时只把该任务设 `in_progress`；任何其他任务保持 `blocked`。

### 4.1 MCP010 当前领取规则

- 当前 010A 已由用户批准并完成真实 Desktop Gate，标为 `done`；
- 成功 receipt 必须保留，第一次失败 receipt 也不得改写；
- 010A 已 done；010B structural source Gate已通过但 Darwin OS memory hard cap deferred；010C source-focused Gate 已完成但视觉子门仍未运行；D/E source Gate 已完成；当前只允许 F 保持 `in_progress`，其 packaged/human/360 子门独立记录；之后每次只将直接后继改为 ready/in_progress；
- 当前工作树有 515 Schema、28 operator entries、111 read + 83 opt-in write = 194 个工具、12 个 Skill（包含每个设计 MCP session 必须先读的 `ponytail-preflight@0.1.0`；历史 `0.1.0` + `primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`uv-pbr@0.2.0` active）；Hero UV 7 个 registered contracts 已接入 Store/Runtime/MCP public `hero_uv_durable_get/prepare`，四个 Hero CAS roots 已纳入 linked/GC，durable prepare/replay/drop/reopen/get **1/1 PASS**；Low provenance 为 candidate-bound 且 `DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`。这些 slice 仅为 `structural_only`，不是 artist unwrap、visual、human、engine、commercial 或 packaged PASS；C source Gate 已通过 contracts、fixed renderer/九 AOV、comparison/typed visual review、MCP image block 和 deterministic raw stdio，D/E source Gate 已通过真实 Operator、AssetPack、UV/PBR/MikkTSpace/embedded-texture 和九 AOV，并有同 cohort packaged D/E structural probes，F source Gate 另通过哈希绑定轮廓目标、Runtime-owned camera reference、方向性边界误差、多 Part `silhouette_part_error_get` 归因表、只读 Viewer 的 AOV/compare/selection/explosion/heatmap 及构建边界，packaged Viewer 另有 read-model/window/core-control smoke；Agentic projection、durable session/checkpoint/RepairIntent prepare/readback 与 CAS-bound RepairIntentRun 另有合同、preflight、空参考 fail closed、Runtime/MCP 重启和隔离持久化检查。当前状态保持 Stage=`camera-calibrated`、`secondary-form-approved=NOT_CREATED`、`FPS-HIGH-05=NOT_PASSED`、`QUALITY_TARGET_NOT_MET`、HQ360=`BLOCKED_REFERENCE_COVERAGE`、proposal=`registered=false`，无 confirm/version/export；attempt35 likeness/receipt/camera 仍失败或不完整；provisional observation package binding、正式 VoiceOver、人评阈值、xatlas/Validator、PBR likeness、export/restart hash和360只写 `NOT_RUN/BLOCKED`。

## 5. FGC-MCP005 已完成记录

### 5.1 已完成 Gate

- `MVP_DELIVERY_PLAN.md` 的 MCP005；
- `MCP_RUNTIME_CONTRACT.md` 的参考导入；
- `SCHEMAS.md`、`DATABASE.md`、`TEST_STRATEGY.md`；
- CAS/Runtime/MCP 当前实现和 `docs/evidence/mcp004/manifest.json`；
- `EXTERNAL_PROJECT_ADOPTION.md` 中 `image-rs/image`、`img2threejs`、`img2css` decision。

MCP005 已完成：

1. `ReferenceEvidence@1`、import/get request/result Schema 与 Runtime records/migration；
2. PNG/JPEG decoder limits、MIME/魔数/截断/超限/hash mismatch、authorized root/outside-root/symlink negative tests；
3. Store/Runtime CAS admission；migration 在 OS 文件锁之后；永久状态不保存原路径；
4. MCP `reference_import/reference_get` 与 `supports_reference_import=true`；
5. `image-rs/image` 只启用 PNG/JPEG，依赖锁已更新；
6. 真实 Codex CLI 隔离 Runtime 导入用户授权 PNG，源 SHA-256 与 CAS object hash 一致；证据只留 hash/尺寸/MIME/授权；
7. Desktop 当前 bridge 诚实记录 `NOT_RUN / unavailable`；
8. 证据、状态账本、handoff 和下一任务已更新。

### 5.2 禁止扩展

图片进入 CAS 不是建模完成。当前 MCP007–009 已打开受限 geometry/GLB、UV/tangent/PBR、固定渲染、limited quality、stable-Part change、immutable version/restore 和 CAS export；仍禁止 Blender、资产下载和远程 Provider。

## 6. MCP006–009 执行摘要

### MCP006（已完成）

先 Schema/validator，再 first-party Skills。MCP006 已完成 44 个 contracts schema、十项 historical registry manifest、十个独立标准 Bundle、trust hash、`skill_list/get` 和只读 resource、DAG/单位/finite/预算 validator、负向 fixture、benchmark receipt、LICENSE/NOTICE/SBOM/provenance 绑定；当前总数为 125 Schema，`primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0` 与 `uv-pbr@0.2.0` 均有 active Runtime consumer。Codex 提交 typed program，ForgeCAD 不调用 LLM。MVP Bundle 用 canonical hash + first-party trust root；不省略许可证/SBOM/provenance，但分发签名/撤销延后。它只证明声明式能力可审计，不证明通用视觉质量。

### MCP007（已完成）

`[transition-v1]` MCP007 已通过 `npm run mcp007:test`：Geometry Worker library/binary 接受 canonical `GeometryProgram@1` primitive-only program，生成确定性 glTF 2.0 GLB；Runtime 生成 geometry candidate/quality report，MCP 通过 authenticated IPC 暴露 `geometry_prepare` 和 `artifact_readback_get`，Viewer read model 读取候选与 artifact lineage。该历史 fixture/receipt 不等于当前 `GeometryProgram@2` detail、九 AOV strict compare、像素相似度或真人高质量结论。

### MCP008（已完成功能核心）

`[transition-v1]` `npm run mcp008:test` 已通过：hash-bound 三种 MaterialZone、UV/tangent/glTF PBR lowering、四个固定 PNG、Runtime readback 和 Three.js GLB canvas。Viewer 只读，不复制状态；该历史四-pass receipt 不替代当前 `AppearanceProgram@2`、九 AOV、strict compare、PBR likeness 或视觉评分。证据：`docs/evidence/mcp008/`、`docs/evidence/mcp009/`。

### MCP009（MVP host golden path 已完成）

`[transition-v1]` `npm run mcp009:test` 已通过 24 个 Runtime tests + 16 个 MCP tests；真实 Codex CLI 已完成十二调用 reference→geometry→appearance→readback→quality→candidate_confirm→version_list→CAS-only export。该历史 `QualityReport@1` 只含明确 `limited` aspect-ratio；`change_prepare` 要求当前 base version、稳定 Part ID、allowlisted operation 和新 typed programs；confirm/reject/restore 保持 immutable/idempotent；`mvp-glb` export 只消费 confirmed quality-passing CAS GLB，返回 output hash 和 receipt，不写任意本机路径。当前 MCP010C/F 已实现 candidate-bound silhouette/landmark/region compare 和 Codex typed visual review，但 attempt35 仍 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，独立真人评分和 retained-candidate packaged Viewer E2E 未运行。证据：`docs/evidence/mcp009/` 与 Stage 0 真值。

## 7. GitHub / Skill / Plugin 纪律

用户允许 GitHub 研究和配置，不等于允许任意安装。必须遵循：

```text
search/read source + release + license
→ classify library/tool/asset/reference-only
→ pin exact revision
→ adoption receipt + LICENSE hash + transitive SBOM
→ isolated malicious/resource/determinism benchmark
→ accepted decision
→ only then edit lockfile/build/package
```

允许优先评估：image-rs/image、gltf-rs/gltf、Manifold、xatlas、mikktspace、glTF-Validator、glTF-Transform。`approved-for-evaluation` 不等于 `adopted`。

禁止安装：BlenderMCP、FreeCAD MCP、任意 Python CAD MCP、Substance 插件/SDK、远程 image-to-3D Provider、自动下载模型权重、GitHub prompt/Skill pack。MCP010E 仅允许 Codex 将计划点名的 CC0 文件一次性下载到本机 adoption cache；逐资产 hash/license/SBOM/provenance 通过后才能编入 first-party 离线 AssetPack。Runtime、安装器和 Viewer 不联网、不调用素材 API。

## 8. 实施纪律

- 保留 dirty worktree，不 reset/clean/checkout 用户修改；
- 文件修改用 patch；不 commit/push/merge，除非用户明确要求；
- 新公开合同先 Schema、生成类型、validator、negative tests；
- Runtime 唯一写库；MCP/Viewer/Worker 不开 SQLite；
- 永久写必须绑定 project/base/candidate/artifact/quality/approval/idempotency；
- 不记录 secret、prompt、原图副本、用户名、绝对路径；
- 不开网络服务、8000、Provider、任意 shell/Python/JavaScript；
- 失败路径不创建版本，不以 fallback 假成功；
- 任何质量数字同时记录阈值、实测值、fixture 和 limitation。

## 9. 验证分类

```text
PASS      当前工作树实际运行成功
FAIL      已运行且失败
BLOCKED   权限、宿主、硬件或外部状态阻断
NOT_RUN   本轮没有运行
```

focused ≠ aggregate ≠ real Codex ≠ packaged ≠ visual ≠ human。必须分别写。历史 receipt 不覆盖当前二进制，文档总结不覆盖 GLB/render/raw report。

共同 Gate：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
npm run contracts:check
git diff --check
```

任务专属命令在对应 evidence manifest 中固定；依赖变更后必须证明 offline build。

## 10. 每轮 handoff

必须更新：

- `CODEX_HANDOFF.md`：实际做了什么、命令/exit、真实运行、blocked/not-run、下一动作；
- `CODEX_TASK_INDEX.md`：只有满足退出条件才变状态；
- `DOCUMENTATION_STATUS.md` 和 capability matrix：当前能力；
- 对应合同、用户指南和 evidence manifest；
- 外部依赖 receipt、THIRD_PARTY_LICENSES/SBOM（若采用）。

## 11. Goal 状态句

完成单个任务：

```text
FGC-MCPxxx completed: all listed exit gates passed on <commit/worktree>; next ready task is FGC-MCPyyy.
```

未完成：

```text
FGC-MCPxxx not complete: <PASS/FAIL/BLOCKED/NOT_RUN evidence>; next safe action is <one action>.
```

MVP 关闭：

```text
ForgeCAD MVP completed for the first hard-surface reference benchmark on <commit/worktree>; universal high-quality image-to-3D and production distribution remain out of scope.
```

## 12. 当前可执行的高质量工具顺序

真实 Codex host 具备 MCP write opt-in 后，按下面顺序调用；每一步把返回的 ID/hash传给下一步，禁止从模型自由猜测 hash。新的 MCP stdio 设计会话必须先成功读取 first-party `ponytail-preflight@0.1.0`；只有 `capabilities_get`、`runtime_status`、`doctor` 可作为无状态诊断例外，不能代替前置读取：

1. `skill_get(ponytail-preflight@0.1.0) → capabilities_get → runtime_status → doctor → operator_catalog_get → skill_list`：先保存 Skill manifest/knowledge canonical hash，再要求 Runtime Ready、同 cohort、catalog digest 一致；当前口径必须是 111 read + 83 opt-in write = 194；Hero UV public `hero_uv_durable_get/prepare` 仍只证明 `structural_only`，RepairIntent staged run 也必须走显式 write opt-in。未完成 `skill_get` 时不得调用 `project_create`、参考、Geometry、Appearance、比较、评审或其他 Skill。
2. `project_create → reference_import → reference_get`：只提交用户授权 PNG/JPEG，记录 project/reference/object hash 和 observed/inferred/unknown coverage。
3. 构造 project/catalog-bound `GeometryProgram@2` detail draft；`geometry_program_hash → geometry_prepare → job_get → candidate_get → artifact_readback_get`，要求 strict `ArtifactReadback@2` 的完整 lineage 和零 integrity failure。
4. `reference_mask_prepare → silhouette_target_get → camera_fit_prepare → silhouette_rig_hash → silhouette_fit_prepare`；fit 返回的 camera 必须与后续 compare 的 camera hash/canonical hash 一致，不一致即停止。
5. 仅在轮廓/结构门允许时提交 `AppearanceProgram@2` 并运行 `appearance_prepare`；AssetPack/UV/tangent/PBR readback 不等于 PBR likeness。
6. `reference_compare_prepare` 必须绑定同一 project/reference/candidate/artifact/camera，生成 `RenderSet@2` 九 AOV strict compare；逐项 `render_pass_get`，再 `visual_review_submit → quality_get`。
7. 任一 strict metric 失败时保留 receipt，只做一次单 Part/detail 受限修正并重跑 readback/九 AOV/strict compare；`QUALITY_TARGET_NOT_MET` 禁止 confirm/export。
8. 只有 strict visible-view 通过后才进入独立 `human_visual_review_submit`；正式真人门当前仍 `NOT_RUN`，不得由 Codex typed review 代替。
9. 真人批准精确 candidate/version 后才可 `candidate_confirm → version_diff → restore_prepare/restore_confirm → export_prepare/export_confirm`，并验证 preview/export/restart 同 hash；这些门当前仍未完成。

`[transition-v1]` `GeometryProgram@1` primitive-only + `AppearanceProgram@1` + `RenderSet@1` 四 pass 只保留 MCP007–009 structural MVP/历史导出兼容。它不是当前高质量路径，不能产出 strict likeness、PBR、human、packaged Viewer 或 360 结论。

真实验收必须另外记录：Codex host 类型、MCP initialize 版本、参考源字节 hash、Geometry/Appearance canonical hash、GLB hash、RenderSet hash、QualityReport、approval receipt、version DAG、Viewer readback、重启后的 hash 和真人评分。任何一项没有运行，都写 `NOT_RUN`；宿主不可用写 `BLOCKED`。

<!-- forgecad-stage0: schemas=662 schema_set_sha256=202e080ec378ddb294eb9c880079dcec5c910b27a1c679034ca34c5a880dcec6 read_tools=131 write_tools=95 total_tools=226 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->
