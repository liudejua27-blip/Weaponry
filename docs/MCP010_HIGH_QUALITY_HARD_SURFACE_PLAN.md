# FGC-MCP010 高质量硬表面参考闭环计划

当前合同面为 **402 个 JSON Schema**；MCP source surface 为 **90 read + 69 write = 159 tools**。CandidateAnimationVfxQuality@2 的 Runtime full 为 **354 passed / 0 failed / 22 ignored**（376 total），MCP full 为 **152 passed / 0 failed / 0 ignored**；真实 Attachment@3 + Quality@2 public full-chain positive fixture 仍 `NOT_RUN`。

2026-08-22 `CandidateAnimationVfxQuality@2` source/structural gate：Contracts **402**；Store focused **3/3**、Store full **112/112**；Runtime focused **6/6**、同源码同 cohort Runtime full **354 passed / 0 failed / 22 ignored**（376 total，115.40s）；MCP focused **4/4**、full **152 passed / 0 failed / 0 ignored**（2.49s）；contracts/runtime/store/MCP joint cargo check **PASS**。旧 `GEOMETRY_WORKER_PROTOCOL` 报告来自 stale Worker binary，已由同源码 Geometry/Render Worker 重建后清除。尚无真实 `Attachment@3 + Quality@2` public full-chain positive fixture，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；当前仅 `structural_only`，visual/artistic/commercial FPS=`NOT_PROVEN`，human/engine=`NOT_RUN`，不推进 stage/confirm/version/export。证据：`docs/evidence/mcp010f/candidate-animation-vfx-quality-v2-durable-source-gate-20260822.json`。

2026-08-22 `CandidateMaterialSurfaceQuality@1` public positive fixture：`Geometry → CandidateTopologyQuality@1 → AppearanceProgram@3 → TextureBuild@2 → SurfaceBake@1 → AppearanceSourceLineage@1 → CandidateMaterialSurfaceQuality@1` 的 `prepare → same-key replay → get → Runtime drop/reopen → restart get` 通过 **1/1（111.72s）**；Runtime focused **5/5**、Store full **74/74**、Contracts **350**。CAS inventory unchanged；stable `artifact_id` 与 GLB object SHA-256、MaterialPack CAS kind 精确区分，合法 UV/tangent rebuild 不计入 geometry-preservation 漂移。该结果仅为 `structural_only`；V2 animated-socket-particles 仍无完整 public `prepare → Store → restart get`，durable end-to-end=`NOT_RUN`/`BLOCKED_FIXTURE_CHAIN`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/candidate-material-surface-quality-public-positive-source-gate-20260822.json`。

最终同 cohort 修订口径：强制 build cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` 下 material focused **5/5（112.63s）**；Runtime full **310 passed / 0 failed / 20 ignored**（330 total，201.91s），且 public material fixture 明确在该 full run 内执行。此前 **111.72s** 仅为 public fixture 单测时长；两者都只支持 `structural_only`，不提升 visual/commercial、human/engine 或 stage/confirm/version/export 状态。

2026-08-22 `FictionalEnergyVfxAnimatedSocketParticlesSequence@2` 双候选 source slice：Contracts **350**；Store V2 focused **2/2**、Store full **74/74**；Runtime V2 仅低层 focused **6/6**、cargo check **PASS**；MCP V2 **3/3**；同 cohort `724278fe8f6777c8b3d07bc5058208aee90aa5c700db5f9284d6297126fa79f6` Runtime full **309 passed / 0 failed / 20 ignored**（191.06s）、MCP full **128 passed / 0 failed / 0 ignored**（1.93s），这些是全量回归，不是 V2 public `prepare → Store → restart get` 正向 fixture。V1/V2 隔离；V2 仅证明 1..16 frame、geometry/appearance 双 candidate/delivery/AnchorSet bridge 以及 Store FK/reachability/idempotence/conflict/rollback 的结构面。完整双候选 public Runtime `prepare → Store → restart get` 正向 fixture 尚不存在，durable end-to-end=`NOT_RUN` / `BLOCKED_FIXTURE_CHAIN`，不能声称正向 durable。该 slice 为 `structural_only`；visual/commercial=`NOT_PROVEN`，human/engine=`NOT_RUN`，stage/confirm/version/export=false。证据：`docs/evidence/mcp010f/fictional-energy-vfx-animated-socket-particles-v2-dual-candidate-source-gate-20260822.json`。

2026-08-22 `TRAILS-21` 当前为 source `in_progress`：已形成动态 animated socket Trails 与 TrailsBloom 的 closed Contracts、versioned Render Worker/Protocol、Store parent/frame 事务，以及 Runtime/MCP prepare/get focused source 实现；新增 animated GLB socket transform projection 的 Runtime-owned durable/replay/restart read-only source Gate。当前源真值为 Contracts **344**、**26/26 active operators**、MCP **79 read + 59 write = 138 tools**；同 cohort `9cc7c11b8309ee7cc76df3d67794c4692c81ff0e8ef064248955ad511b8ca388` Runtime full lib **307/307**（0 failed，另 20 ignored）、binary **1/1**，合计 **308 pass**，Store full **72/72**、显式固定 cohort 的 MCP full **125/125**。该 projection Gate 不等于 animated-socket Particles/Trails/TrailsBloom 正向集成；动态 VFX 链仍缺完整正向 durable `prepare → Store → restart get` 集成测试和独立回执，因此不得宣称 durable end-to-end PASS。整体保持 `structural_only`，不推进 stage/confirm/version/export，视觉/艺术/商业 FPS=`NOT_PROVEN`，人评/引擎=`NOT_RUN`。证据：`docs/evidence/mcp010f/game-weapon-animated-glb-socket-transform-projection-source-gate-20260822.json`。

2026-08-21 `TOPOLOGY-QUALITY-16` 已把 durable 多轮生产轴推进到 `draft → gray-model → topology`：candidate-wide `CandidateTopologyQuality@1` 精确绑定全部 Part、GLB/V2 readback、GeometryCandidateEvidence、program/catalog/config，并以 blocked/passed 报告驱动独立 stage transition；prepare 本身不 confirm/version/export。当前口径为 **303 schemas / 26/26 active operators / 71 read + 51 write = 122 tools**，Runtime full 为 **268 pass / 0 fail / 20 ignored**。本轮只证明每 Part ≤512 faces 的 evaluated triangulated GLB 结构门，authoring cage coverage、edge flow、艺术/视觉/人评和商业引擎成品仍未证明；下一子门为 source topology candidate → derived appearance candidate 的 `CandidateMaterialSurfaceQuality@1`，再由 V2 stage head 完成 `topology → material-surface` 重绑定。

2026-08-21 当前 MCP010F `BEVEL-CANDIDATE-14` source/runtime durable slice 已实现：closed `GeometryModifierApplyRequest@2 → Result@2` 把 direct `authoring-mesh@1` 的单 stable edge `bevel@2` 接到 exact current-head candidate，Store 事务复验 source/head/derived/sidecar，幂等、重启与 @1 回归通过；同 cohort Geometry/Render Worker 对 source/derived 使用同一固定相机，各重复两次完整九 AOV并通过 byte-exact、silhouette/bbox/centroid 非退化及 normal/wireframe detail-change Gate。当前口径为 **293 schemas / 69 read + 49 write = 118 tools**。这是 bounded structural visible-detail handoff，不是 arbitrary edge-chain bevel、参考 likeness、PBR、人评、确认/版本/导出或商业质量 PASS。

数值口径：当前 source 为 **402 个 JSON Schema / 26/26 active operators / 90 read + 69 write = 159 tools**；下文 375/149、344/138、334/134、334/132、329/132、324/130、319/128、314/126、308/124、303/122、298/120、293/118、291/118、290/118、284/116、278/114、271/112、264/110、257/108、231/100、229/99、227/98、221/96、215/94、210/92、204/91、201/90、197/90、195/90、191/90、187/89、177/84、175/83、173/82、170/80、168/79、166/78、164/78、162/77、160/76 均是 historical prior snapshot，即使历史文案保留“当前/current”字样也不得覆盖本口径。

2026-08-21 `GAME-ENGINE-IMPORT-13` engine-import subgate：evidence-only 的真实 Godot `4.7.2.stable.official.ed1daf0bf` headless importer 对 4 个 GLB scene（LOD0/1/2/animated）通过。LOD triangles `304 > 176 > 112`，每 scene 5 meshes/相同 materials/6 non-rendering sockets parent/local TRS exact；animated 10 glTF channels→2 Godot semantic tracks，cross-loader t0/half-duration TRS exact，两个 named sockets follow；5 个 `BoxShape3D` collision sidecar readback exact。static/animated 由独立 Runtime source cohorts 提供，不创建 Runtime durable Godot link。commercial engine=false，Unity/Unreal、physics/hitbox、visual/human、confirm/export `NOT_RUN/false`，quality `structural_only`；总体质量目标与缺失绑定阻断不变。合同/tool 计数为 **291 schemas / 69 read + 49 write = 118**。

2026-08-21 当前 MCP010F `GAME-ANIMATED-GLB-SOCKETS-12` source/runtime/MCP durable structural slice 已实现：candidate-bound `MechanicalAnimationGlbReceipt@1` animated LOD0 source 与 delivery LOD0、AnchorSet 精确绑定，derived animated GLB 物化六个稳定命名 empty socket nodes；源/派生动画 projection、animations/channels/samplers、source static/renderable projection 与 BIN bytes exact。Runtime-owned CAS 恰有两个（derived animated socket GLB + materialization receipt），Three.js r185 `GLTFLoader` 已验证 10 tracks exact projection 与 half-duration 两个 socket parent-follow。该结果不证明 Unity/Unreal/Godot、runtime pivot、functional weapon semantics、视觉/真人、确认/版本/导出或商业品质；总体 `QUALITY_TARGET_NOT_MET` / `INCOMPLETE_TRUTH_BINDING` / camera `MISMATCH` / `BLOCKED_REFERENCE_COVERAGE` 保持。receipt：`docs/evidence/mcp010f/game-weapon-animated-glb-socket-materialization-source-gate-20260821.json`、`docs/evidence/mcp010f/threejs-game-weapon-animated-glb-socket-consumer-20260821.json`。

2026-08-21 当前 MCP010F `GAME-GLB-SOCKETS-11` source/runtime durable structural slice 已实现：delivery LOD0/1/2 各生成一份派生 GLB，每份 exact six stable named empty nodes，并保持源 renderable projection 与 BIN byte-exact；四个 owned CAS、SQLite exact-three-child binding、幂等/冲突/rollback/restart Gate 与 Three.js r185 `GLTFLoader` 三 LOD 消费通过。该结果不证明 Unity/Unreal/Godot、动画 socket、runtime pivot、视觉/真人、确认/版本/导出或商业游戏资产质量。

2026-08-21 当前 MCP010F `VFX-TYPED-TRAILS-BLOOM-10` source/runtime durable same-cohort slice 已实现：严格重建现有 typed trails 并以固定 profile 输出独立 `trail-emissive-source` / `trail-bloom-contribution`，同时保持原 trail 三 pass、基础九 AOV、原 HDR Bloom 与 particles byte-exact；CAS/SQLite、幂等、冲突拒绝和 Runtime restart Gate 通过。该输出是 RGBA8 review evidence，不是 FP16；GLB socket、商业引擎、foreign-cohort、人评、确认/版本/导出与视觉质量仍未通过。

2026-08-20 当前 MCP010F `bevel@2` source/Skill slice 已实现：direct `authoring-mesh@1` 的单 stable source edge 在 closed convex triangle/quad、端点 valence=3 的 P0 范围内执行 1–4 segment/profile bevel。segments=1/3 的 closed cube 为 16/24 triangles；strict GLB、solid、determinism、edge/width/profile variation 以及 boundary/non-manifold/multi-edge/oversize/unknown executable field fail-closed 通过。它不进入 Modifier/Agentic/Repair；当前 weapon candidate、多视图、PBR、人评、package/live、export-restart 和 HQ360 均未推进。

2026-08-20 当前 MCP010F `energy-core@1` source/Skill slice 已实现：固定 Worker 生成四种有界 closed component，四节点 fixture 为 4 semantic Parts / 768 triangles，strict readback 的 boundary/non-manifold/winding 均为 0。负 inner radius、solid 非零 inner radius、关系/预算/hash/unknown executable fields fail closed；MCP/Runtime Agentic allowlist 和 typed numeric patch 闭合。Worker 60/60、Runtime 233/0/12 ignored、MCP 86/86 与 raw stdio 25/25 Gate 通过；当前 weapon candidate、多视图、PBR、人评、package/live 与 HQ360 均未推进。

2026-08-20 当前 MCP010F `vent-array@2` source/Skill slice 已实现：产品自有 direct-topology Worker 生成一个 connected/watertight 的贯穿开槽面壳，前后 bevel 深度环、closed backing sub-solid、`backing_gap_m`、逐槽/component/312-triangle 精确预算、strict GLB readback/lineage/determinism、v1 compatibility 与 Skill trust Gate 均通过。backing 仍是同一 PartOutput 内的几何 component，不是独立 semantic Part；当前武器 candidate、多视图、PBR、人评、package/live 与 HQ360 均未推进。

2026-08-20 当前 MCP010F `panel@2` source/Skill slice 已实现：闭合 grammar 真实生成 recessed floor、border band、分段 bevel 和 support loops，保留 `panel@1` 兼容性，并经 strict GLB readback、lineage、catalog/hash、资源预算、负向输入、first-party Skill trust 与 license/SBOM Gate。该 operator 目前是独立语义 Part grammar，不是任意宿主 mesh 的 BMesh inset；需通过现有 typed Boolean/Part DAG 组合到主体。尚无当前武器候选的多视图、PBR、人评、package/live 或 HQ360 视觉证据。

2026-08-20 当前 MCP010F Mechanical Animation Viewer slice 已在 candidate/artifact-bound、认证只读的 clip inventory/detail 上加入 scheduled single-tick Runtime frame。Viewer 仅把同 cohort 双 Worker 已验证的 rigid Part delta 应用到 exact-one identity GLB Part owner；embedded animation、Bone/SkinnedMesh、owner 歧义与异步旧响应 fail closed。UI 状态不持久化，Viewer 不本地求值、不自动连续播放、不调用 prepare/confirm；这只是 structural discrete-frame projection，不是动画编辑、Armature/skin/IK/NLA/F-Curve/GLB animation、Blender parity、视觉质量或 package/live PASS。

2026-08-19 当前 MCP010F Authoring Mesh Edit Prepare slice 已实现：显式 write-opt-in 工具只把通过 Runtime 重新计算、固定 Worker compile、strict readback 和 current-head Gate 的 edit 暂存为 reviewable candidate；Store 原子写 Job/evidence/audit/idempotency，不执行 confirm/version/export。该 source structural Gate 不构成视觉质量、完整 BMesh/editor、Python/plugin 或 Blender parity。

2026-08-19 当前 MCP010F Mechanical Animation Clip slice 已实现：显式 write-opt-in `mechanical_animation_clip_prepare` 将 bounded caller-authored rigid action 写为 Runtime-owned immutable CAS clip 与 SQLite Link；写入前由实际 Geometry Worker 对 durable source GLB 做两次 full-byte replay，并要求同一非空 build cohort。默认只读 `mechanical_animation_clip_get` 可跨 Runtime 重启回读，`mechanical_animation_clip_preview_get` 只允许 immutable schedule 内单 tick，并对 transient derived frame 做两次 exact Worker replay且不写 candidate/version/CAS。该 structural Gate 不提供 Armature、skin、IK、NLA/F-Curve/driver、角色时间轴、GLB animation channel、Blender add-on 或 Python runtime，也不升级视觉质量。

2026-08-19 当前 MCP010F Render Evidence Replay slice 已实现：Runtime 在不写任何产品状态的前提下，对 exact candidate-bound GLB 做 strict readback，并用实际 fixed Render Worker 同 cohort 两次重放固定相机，只有九 AOV 的 source/first/repeat PNG bytes 和 RGBA8 pixels 全部完全一致才通过。该结构闭环不表示视觉质量、PBR likeness、跨平台 determinism 或 Blender renderer parity。

2026-08-19 当前 MCP010F Mechanical Pose Geometry Preview slice 已实现：Runtime 对 caller-authored rigid rest/action 与 durable candidate cohort 做 exact binding，按 `PoseWorld × inverse(RestWorld)` 为纯 Part outputs 派生 transient `transform@2` sinks，经 fixed Worker 重新 hash/compile/strict readback，并保持 CAS/SQLite/candidate/version 零写入。该 structural preview 不证明原资产 rig/pivot provenance、Armature/skin/动画或任何视觉质量。

2026-08-19 当前 MCP010F Subdivision artifact-lineage sidecar slice 已实现：显式 Runtime prepare 把已通过 exact artifact replay 的 lineage 写为 immutable CAS sidecar，独立 SQLite link 固定 request/candidate/artifact/readback/evidence/node hashes；默认 getter 只读、重启稳定且不懒写。该 structural persistence 仍不产生跨版本 element identity、视觉质量、Cycles/EEVEE parity、package/live、人评、PBR、export/restart 或 360 PASS。

2026-08-19 当前 MCP010F Subdivision artifact-lineage source slice 已实现：Runtime-owned read projection 对 exact durable V2 geometry evidence、strict readback 和 fixed-Worker full-GLB deterministic replay fail closed，并将 control-root evaluated quads 映射到唯一 direct source primitive 的 local triangle ranges。该 slice 没有修改 `ArtifactReadback@2`、没有 getter 懒写、没有持久 sidecar，也不证明 Blender/OpenSubdiv parity、视觉、人评、package/live、PBR、export/restart 或 360。

2026-08-19 当前 MCP010F crease-aware Subdivision source slice 已实现：active `subd-cage@2` 将 bounded integer edge crease 真正交给 fixed Geometry Worker 求值；`geometry_program_hash` 负责 closed request/policy/topology/program binding，`geometry_prepare` 负责 actual GLB/CAS candidate/strict readback。只完成 regular-grid structural capability，不解决参考 likeness、渲染、PBR、人评、package/live、export/restart 或 360；`QUALITY_TARGET_NOT_MET` 等现有视觉账本不变。

2026-08-19 historical MCP010F Blender clean-room Boolean Operand Lineage source slice 已实现：该 slice 当时为 164 个 JSON Schema、19/19 active operators、45 read + 33 opt-in write = 78 tools。`BooleanOperandLineageRequest@1 → BooleanOperandLineage@1` 通过 ForgeCAD 自有固定 Geometry Worker（vendored Manifold bridge）暴露 bounded operand/evaluated-face runs，并由 Runtime 从输入 program 独立重算 Boolean operation、左右 operand 与递归 source lineage 后在 MCP 只读回路重验；没有新依赖、Skill、可执行插件或任意脚本。face ID 仅为求值后 planar-face identity，不是原始建模面，也不持久化到当前 GLB；视觉、人评、PBR、export/restart、360 状态不变。receipt：`docs/evidence/mcp010f/blender-boolean-operand-lineage-source-gate-20260819.json`。

2026-08-19 historical MCP010F Blender clean-room Render Evidence Integrity source slice 已实现：162 个 JSON Schema、19/19 active operators、44 read + 33 opt-in write = 77 tools。`render_evidence_integrity_get` 是 exact-hash-bound、1 MiB、只读 current-cohort 投影，深度验证 ArtifactReadback、camera、RenderSet、comparison、quality、九 AOV 和 comparison mask；不新增 renderer/Worker/Skill/operator/dependency，不宣称 Cycles/EEVEE/OCIO parity。attempt35 的 camera mismatch、AOV hash missing、threshold/unrun/material predecessor 缺口仍保留，视觉/真人/PBR/export-restart/360 不升级。

2026-08-19 historical MCP010F Blender clean-room Mechanical Pose Sequence Preview source slice 已完成：160 个 JSON Schema、19/19 active operators、43 read + 33 opt-in write = 76 tools；现有只读 `mechanical_pose_evaluate` 增加最多 16 个严格递增 tick 的 sequence preview branch，每个 sample 复用单 tick 的 Runtime semantic recomputation 与 hash validation。无新 tool/operator/Skill/Worker/dependency、无 Runtime 持久写入，也无 Armature/skin/timeline/animation asset。receipt：`docs/evidence/mcp010f/blender-mechanical-pose-sequence-preview-source-gate-20260819.json`。该 structural Gate 不升级视觉、package/live、人评或 360 结论。

2026-08-18 historical Parametric Group v2 source：该 slice 当时为 158 个 JSON Schema、19/19 active operators、43 read + 33 opt-in write = 76 tools；三个固定 first-party group template 仅通过现有只读 `geometry_program_hash` lowering，无新 tool/operator/Skill/dependency、无 Runtime 持久写入。receipt：`docs/evidence/mcp010f/blender-parametric-group-v2-source-gate-20260818.json`。

2026-08-18 historical Mechanical pose source：该 slice 当时 manifest 为 **156 个 JSON Schema**、19/19 active Operator、43 read + 33 opt-in write = 76 tools。它增加 candidate/artifact/readback/Part/source-node 全绑定的 `MechanicalRestFrame@1`、有限 `MechanicalPoseAction@1` 与默认只读 `mechanical_pose_evaluate`；只返回 deterministic local/world TRS projection，不调用 Worker、不 materialize 几何、不写状态。Armature/skin/IK/NLA/F-Curve、package/live/视觉/真人仍未实现或未运行；`QUALITY_TARGET_NOT_MET`、`INCOMPLETE_TRUTH_BINDING`、camera `MISMATCH` 与 `HQ_360=BLOCKED_REFERENCE_COVERAGE` 不变。

2026-08-18 historical RenderProfile/AOV source：该 slice 当时 manifest 为 **152 个 JSON Schema**、19/19 active Operator、42 read + 33 opt-in write = 75 tools。ForgeCAD 自有固定 CPU software renderer 增加 closed `RenderProfile@1`，并让 Worker 与 Runtime `RenderSet@2` 精确绑定 profile/AOV/color-pipeline/ID-palette hashes；beauty 是唯一 display-color pass，其余 AOV 为 non-color data。它不提供 Blender/Cycles/EEVEE/OCIO/GPU/EXR parity，package/live/视觉/真人仍未通过。

2026-08-18 historical Modifier evaluation v2 source：该 slice 当时 manifest 为 **149 个 JSON Schema**、19/19 active Operator、42 read + 33 opt-in write = 75 tools。Modifier evaluation v2 只提供 Runtime-owned canonical authoring/effective/program/policy/catalog/cache hash、dirty reason 与 deterministic reuse decision；不编译 mesh、不持久化 cache、不创建 candidate/version，也不证明 package/live、视觉或 Blender parity。其下一原子项 Subdivision v2 已由上一段完成；`QUALITY_TARGET_NOT_MET`、`INCOMPLETE_TRUTH_BINDING`、camera `MISMATCH` 与 `HQ_360=BLOCKED_REFERENCE_COVERAGE` 保持不变。

2026-08-18 historical Blender TopologySnapshot source slice：该 slice 当时为 **146 个 JSON Schema**、42 read + 33 opt-in write = 75 tools。`topology_snapshot_get` 是 Runtime-owned、candidate/hash-bound 的单 Part evaluated GLB triangle topology 完整投影，限制 512 faces/1536 V-E-corner/1 MiB，超限 fail closed；它覆盖现有 SubD cage 与 Boolean output，但明确不是 authoring cage、BMesh、跨版本稳定 ID 或视觉质量证据。其后续 bounded bevel/normal 已按上方路线完成。

2026-08-18 historical Blender reference-only Modifier Stack source slice：该 slice 当时源码为 **144 个 JSON Schema**、41 read + 33 opt-in write = 74 tools。`GeometryModifierStackRequest@1` / `GeometryModifierStackProgram@1` 只把有序 transform/mirror/array lowering 到现有 GeometryProgram@2，保持 `structural_only`、Runtime 唯一写者和 approval boundary；它不完成通用 BMesh/Subdivision/Bevel/Normal、EEVEE/Cycles、动画骨骼或 Python 生态，也不改变 `QUALITY_TARGET_NOT_MET`。

2026-08-17 Reference Visual Structure historical source slice（仍属 `FGC-MCP010F`）：该 slice 当时源码为 **144 个 JSON Schema**、41 read + 33 opt-in write = 74 tools。`ReferenceVisualStructure@1` 作为 `SilhouetteTarget@1` 的可选嵌套合同，强制全局轮廓优先、视觉几何而非功能零件、允许区域 overlap/shared boundary，并保存 continuity group、layer、depth policy 与开放 line-flow；轮廓细化会使结构 review 失效。它只进入 Runtime CAS reference evidence，不创建 candidate，不解锁 detail/PBR/confirm/export，真实视觉仍 `QUALITY_TARGET_NOT_MET`。

2026-08-17 Fictional Energy Rifle Profile source slice（仍属 `FGC-MCP010F`）：新增受限原创游戏美术 Profile/Plan 合同，仅允许 `fictional-game-asset` + `nonfunctional_asset=true`，并复用现有 PDK kit/产品自有 Operator；不引入脚本、插件、模型调用或 Runtime 持久写入。receipt：`docs/evidence/mcp010f/fictional-energy-rifle-profile-source-20260817.json`。这是 source/structural authoring aid，不是 visual likeness/PBR/human/export/restart/360 证据；该 slice 当时合同 144、工具 74，质量真值仍 `QUALITY_TARGET_NOT_MET`，`HQ_360=BLOCKED_REFERENCE_COVERAGE`。

2026-08-17 weapon joint-multiview source Gate（仍属 `FGC-MCP010F`）：针对用户激光步枪参考板，新增固定 SubjectCoordinateFrame、正交 `CameraCalibration@2`、六向 `CameraRigCalibration@1` 与 `OptimizationIntent@2`/`OptimizationEvaluation@2`/`OptimizationResult@2`。Runtime 对所有视图使用同一 candidate 与一次 batch render，按视图/指标产生 weighted 与 worst loss；只有所有视图 non-regressing、primary 严格改善、至少三视图严格改善且 aggregate 严格下降时才准备 promotion。该切片只证明合同/Runtime/MCP source Gate：合同 138，工具 41 read + 33 opt-in write = 74，receipt 为 `docs/evidence/mcp010f/weapon-joint-multiview-optimization-source-20260817.json`；完整 Runtime 因隔离 Geometry/Render Worker 不可用而未全绿。尚无真实多视图 likeness、PBR、人评、export/restart 或 360 证据，当前质量仍 `QUALITY_TARGET_NOT_MET`，完整覆盖继续 `BLOCKED_REFERENCE_COVERAGE`。

2026-08-17 orchestrator dispatch closure（仍属 `FGC-MCP010F`）：`design_stage_run_prepare`/`design_composition_prepare` 已进入 Runtime typed IPC dispatch，真实授权参考隔离 probe 以同 cohort `906c8decb7aec90e8854bb0b7eb4d650dd3895851b556a62a1c8d16557aa17c8` 完成 1 个 stage child ActionRun、`view_spec`/observation 转发、Runtime `RuntimeParameterPatch@1` materialization 与 deterministic batch replay；source candidate 未变，未 confirm/version/export。该修复关闭的是 transport dispatch 漏洞，不是完整 Observe→Plan→Repair→Promote，也不提升 `QUALITY_TARGET_NOT_MET`；`HQ_360` 仍受参考覆盖阻断，live restart、真实多视图、PBR、人评与 export/restart 尚未运行。receipt：`docs/evidence/mcp010f/dev-app-install-orchestrator-dispatch-fix-20260817.json`、`docs/evidence/mcp010f/current-source-real-reference-orchestrator-dispatch-fix-20260817.json`。

2026-08-17 PDK v0 source slice：在不改变 Runtime 唯一写者和无任意脚本边界的前提下，新增 `ParametricDesignKitRequest@1`/`ParametricDesignKitProgram@1`，并经只读 `geometry_program_hash` 展开六类窄范围 macro（housing/panel/frame、vent、joint、sensor）。输出固定 Catalog 的单节点 `GeometryProgram@2`、intent/program/input hash 与 Part/MaterialZone/parameter source map；全部尺寸、倒角、厚度、槽位/环距、位置/旋转、segment/count 关系由 Runtime fail closed。完整 source Gate 通过后，Dev.app 已以同一源码 cohort `6f00a58a…ed50` 重建/安装，隔离 package probe 与 packaged PDK read-only round-trip PASS。该 source/package slice 仅提升“设计意图→有界 GeometryProgram”的可编排性，仍需后续 `geometry_prepare → strict readback → multi-view compare → PBR → human → export/restart`，且不改变当前 `QUALITY_TARGET_NOT_MET`/`BLOCKED_REFERENCE_COVERAGE`。receipt：`docs/evidence/mcp010f/parametric-design-kit-v0-source-gate-20260817.json`、`docs/evidence/mcp010f/dev-app-install-pdk-v0-20260817.json`、`docs/evidence/mcp010f/dev-app-probe-pdk-v0-20260817.json`、`docs/evidence/mcp010f/dev-app-probe-pdk-packaged-read-only-20260817.json`。

2026-08-17 current package raw manifest/OptimizationJob recheck：最终 Dev.app cohort `ce45110e3a5e6eaa5b5283e61f430e2338c7f06a2d09f4e75d4a21cb924f6a86` 的隔离 raw stdio probe 已将 `tools/list` 与生成 source manifest exact-match（41 read + 33 opt-in write = 74，manifest canonical `7ced259e…c526b609`），并完成 39-evaluation `OptimizationJob`（32 coarse + 4 mid + 3 final），Job/result=`succeeded/done`、proposal=`blocked-no-improvement`、`strict_improvement=false`，未 confirm/version/export。新 receipt：`docs/evidence/mcp010f/optimization-raw-manifest-dynamic-20260817-receipt-v2.json`。这是当前包的结构/IPC recheck，不升级 `QUALITY_TARGET_NOT_MET`、live restart、PBR、人评、export/restart 或 360 状态。

2026-08-17 P2 typed surface-finish package：`hard-surface-finish-v1` 已随当前源码安装为 Dev.app cohort `ce45110e3a5e6eaa5b5283e61f430e2338c7f06a2d09f4e75d4a21cb924f6a86`；同 cohort package/probe PASS，receipt：`docs/evidence/mcp010f/dev-app-install-parametric-surface-finish-20260817.json`、`docs/evidence/mcp010f/dev-app-probe-parametric-surface-finish-20260817.json`。该 package 仍只证明 typed surface-finish transport；不解除 `QUALITY_TARGET_NOT_MET`、live Desktop restart、PBR/人评/export/restart 或 `BLOCKED_REFERENCE_COVERAGE`。

2026-08-17 P2 typed surface-finish source Gate：`RuntimeParameterPatch@1` 新增 `hard-surface-finish-v1`，允许 Panel 的 `thickness_m/bevel_m` 和 SurfaceShell 的 `thickness_m` 进入 Runtime-owned 单 Part bounded repair；Panel `thickness <= size.z`、`2*bevel < min(size.x,size.y)` 与混合参数族均由 Runtime fail closed。receipt：`docs/evidence/mcp010f/parametric-surface-finish-parameter-patch-source-gate-20260817.json`。这只补齐可执行 typed surface control，不等于 Parametric Design Kit v0、视觉 likeness/PBR、人评、package/live restart 或 HQ_360 完成。

2026-08-17 P2 camera/observation binding follow-up：RenderSet@2 现在显式携带 CAS `camera_object_sha256`，RepairIntent 生产者与真实探针补齐 durable `observation_sha256`；最新 Dev.app cohort `18c9fb86cafc7e7baf2356e2efe9db404e6530fdeae54d33c0a9beba94fbae40` 的真实授权参考回归已完成 render/compare/evaluate，严格视觉门仍 `QUALITY_TARGET_NOT_MET`，所以 RepairIntentRun blocked、未 confirm/version/export。receipt：`docs/evidence/mcp010f/real-reference-repair-intent-run-observation-cas-20260817.json`。

2026-08-17 历史 slice 真值：144 个 JSON Schema；Stage 0 工具面为 41 read + 33 opt-in write = 74。

2026-08-17 P2 RepairIntent run source Gate：`repair_intent_run_prepare` 验证 Runtime-owned CAS `RepairIntent@1` 与 session/candidate/observation/evidence/reference/camera exact binding，限定单 Part `bounded-repair`，并复用既有 `compile → readback → render → compare`；只返回 reviewable/blocked 的 staged candidate，禁止 intent 覆盖、confirm、version 与 export。source/contract/full Gate 与最终 Dev.app packaged positive transport 通过；相机 CAS 与 observation hash 修复后，真实参考已完成 `render → compare → evaluate`，随后按严格视觉门 `QUALITY_TARGET_NOT_MET` blocked；receipt：`docs/evidence/mcp010f/repair-intent-run-source-gate-20260817.json` 与 `docs/evidence/mcp010f/real-reference-repair-intent-run-observation-cas-20260817.json`。这不是通用 orchestrator 或 Repair 应用完成证明，live/真实多视图与视觉质量仍未通过。

2026-08-17 P2 package Gate：最新 Dev.app cohort `18c9fb86cafc7e7baf2356e2efe9db404e6530fdeae54d33c0a9beba94fbae40` install PASS；真实授权参考 positive `repair_intent_run_prepare` transport 已完成至 evaluate 后按严格视觉门 blocked，live Desktop restart 仍未运行；旧 cohort `ad4837…` receipt 按历史范围保留。

2026-08-17 P1 per-view binding hardening：target/mask 评估现在必须绑定同一 ViewSpec，核心视图 kind/source_view、landmark/region 唯一性、unknown confidence 与 region 边界由 Runtime/schema 双层 fail closed；source receipt：`docs/evidence/mcp010f/reference-view-spec-binding-hardening-source-gate-20260817.json`。这仍是合同/Runtime Gate，不是实际多视图或视觉质量通过。

2026-08-17 packaged P1 rebind：当前源码已重建为 cohort `c4880d38e184624bd8474ef6b9c4b6fcae33c08881876e40c7b21c803bf60242`；隔离 packaged raw stdio 的 9 AOV、确定性重复与 synthetic export/restart hash 通过，证据：`docs/evidence/mcp010f/reference-view-spec-binding-hardening-packaged-export-restart-20260817.json`。不把 synthetic 结构门写成 robot likeness，也未自动重启现有 Desktop MCP。

2026-08-17 core HQ coverage hardening：完整 360 声明现在 fail closed，必须包含 `front/back/left/right/rear-three-quarter` 五个 identity views；补充 perspective/top/material/detail 不得替代。Runtime/schema/Agentic negative fixture/focused tests 通过，receipt：`docs/evidence/mcp010f/core-hq-reference-coverage-contract-gate-20260817.json`。这不是实际多视图 likeness 或 HQ_360 PASS。

2026-08-17 annotation-readiness source Gate：自动 mask/flood-fill 仍是 exploratory observation，不能直接成为 benchmark target。Runtime 现在要求显式 `user_confirmed=true` contour、observed Part regions、至少 3 个 observed target/view landmarks、observed view regions 和 canonical camera exact binding，才输出 `READY_PARTIAL_VIEW`；否则比较和 QualityReport 保持 `BLOCKED_USER_CONFIRMATION_REQUIRED`/`QUALITY_TARGET_NOT_MET`。focused/full MCP010F 与 release baseline 通过，receipt：`docs/evidence/mcp010f/reference-annotation-readiness-source-gate-20260817.json`。这只是单视图标注资格边界，不代表真人评审、robot likeness、PBR、export/restart、Desktop restart 或 HQ_360。

2026-08-17 annotation-readiness package Gate：P1 变更已构建为 Dev.app cohort `896105d3ab204babd415738bf66f572ea5c4be41df1ee83a26da3deed6bb42c7`，同 cohort install/probe PASS；现有 live MCP/Runtime 不会被本步骤重启，故 real Codex authoring gate 仍待用户安全重启。

2026-08-17 live mismatch Gate：安装新包后，现有 stdio 仍使用旧 Runtime/MCP cohort 且 `build_cohort_match=false`；旧连接仍广告 writes，因而在 Codex Desktop 重启并重查能力前不允许设计写入。该状态只阻断 live action，不改变 source/package Gate 或 `QUALITY_TARGET_NOT_MET`。

2026-08-17 per-view annotation lineage source Gate：`ReferenceCanvas@1` 每个 view 现在可以显式携带 `ReferenceViewSpec@1`、silhouette target/mask CAS hash 与 camera canonical hash；Runtime 在 authoring、evaluation、cross-view proposal compare 处做同 view/reference exact binding，RepairIntent 必须覆盖 `coverage.supplied_views` 的每个 kind 且只允许一次，projection 也回读这些 hash。contracts、negative fixtures、Runtime/MCP focused tests、Vite/桌面 source Gate 与 `script/test_mcp010f.sh` 通过；当前没有真实多视图授权参考的新 receipt，故仍不得声称 likeness/HQ，`QUALITY_TARGET_NOT_MET` 与 `HQ_360=BLOCKED_REFERENCE_COVERAGE` 保持。

2026-08-17 typed pose-control source Gate：`SilhouetteRig@1`/`SilhouetteRigHashRequest@1` 现支持 `rotation_x/y/z`、`radian`、±2π bounded validation；Runtime 只在最终 `forgecad.geometry.transform@2` 输出层应用 delta，保留 authored source Part 与 mirror/array lineage。Primary Form action 可请求受限 `head-rotation-y` 等参数，错误单位、范围和非有限输入 fail closed。Runtime/MCP focused tests 与 `script/test_mcp010f.sh` source Gate 通过；默认 detail Rig 仍 36 controls、自动搜索仍固定 40 geometry evaluations，姿态控制未静默改变预算。该 Gate 仅是 typed source/materializer 能力，不是姿态对真实机器人参考的 likeness PASS；质量仍 `QUALITY_TARGET_NOT_MET`，Stage 0 历史 camera/binding、PBR、人评、export/restart/360 状态保持原账本。

2026-08-16 Primary Form Part-namespace repair：Part correction probe 原先只允许 `chest-shell`、肩甲和小腿，导致 canonical observation 归因出的最高误差 Part `pelvis` 在进入 Runtime 前被错误阻断为 `no bounded parameter namespace`。现已把 detail fixture 的全部语义 Part 显式加入 probe 的 bounded namespace（包括 pelvis/hip/thigh/knee/head/torso/limbs/detail sinks），并保持 Rig/参数仍由 Runtime 解释。源码探针语法与 `git diff --check` 通过；旧阻断 receipt `docs/evidence/mcp010f/supplemental/part-correction-camera-binding-fix-pelvis-20260816.json` 保留。

2026-08-16 canonical-camera pelvis repair：在 cohort `125df766029d3568c4a5724838ea342a18f29651566b625758cc6188d25059b6` 上，`pelvis` automatic Part correction 完成 64 evaluations、`accepted + strict_improvement`，acceptance/VisualEvidence/RenderSet 使用 canonical camera `2cd35f435839f80e364a15a6930ffe37c2145925d6a5b455ccffd30873953736`，拟合相机 `04e27c1b…58ad0a` 仅留在 fit evidence。receipt `docs/evidence/mcp010f/supplemental/part-correction-camera-binding-fix-pelvis-v2-20260816.json`：baseline compare IoU `0.741047132807`/Boundary F1 `0.328765122610` → proposal IoU `0.745160673820`/Boundary F1 `0.334696163978`，`QUALITY_TARGET_NOT_MET`、candidate staged、未 confirm/version/export，persistent side effect=false。

2026-08-16 canonical-camera hip-pair repair：同一 cohort 的 `hip-pair` automatic Part correction 通过 64 evaluations、`accepted + strict_improvement` 与 canonical camera exact-match；receipt `docs/evidence/mcp010f/supplemental/part-correction-camera-binding-fix-hip-20260816.json` 记录 baseline IoU `0.741047132807`/Boundary F1 `0.328765122610` → proposal IoU `0.742876384452`/Boundary F1 `0.330228088626`，仍 `QUALITY_TARGET_NOT_MET`、staged、未 confirm/version/export。该局部改善小于 pelvis/chest，不应单独晋升为当前最佳 candidate。

2026-08-16 canonical-camera upper-arm-pair repair：同一 cohort 的 `upper-arm-pair` automatic Part correction 完成 64 evaluations、`accepted + strict_improvement` 和 canonical camera exact-match，但几何收益接近零；receipt `docs/evidence/mcp010f/supplemental/part-correction-camera-binding-fix-upper-arm-20260816.json` 记录 baseline IoU `0.741047132807`/Boundary F1 `0.328765122610` → proposal IoU `0.741042819691`/Boundary F1 `0.330420176973`，仍 `QUALITY_TARGET_NOT_MET`、staged、未 confirm/version/export。由于 IoU 轻微回退，不把该 trial 视为当前最佳。

2026-08-16 canonical-camera shoulder-armor-pair repair：同一 cohort 的 `shoulder-armor-pair` automatic Part correction 完成 64 evaluations、`accepted + strict_improvement`、RenderSet/VisualEvidence canonical camera exact-match；receipt `docs/evidence/mcp010f/supplemental/part-correction-camera-binding-fix-shoulder-armor-20260816.json` 记录 baseline IoU `0.741047132807`/Boundary F1 `0.328765122610` → proposal IoU `0.744097146194`/Boundary F1 `0.338944948938`，仍 `QUALITY_TARGET_NOT_MET`、staged、未 confirm/version/export。

2026-08-16 Primary Form canonical-camera acceptance repair：在源码 cohort `125df766029d3568c4a5724838ea342a18f29651566b625758cc6188d25059b6` 上，Runtime 已将 `primary_form_repair_prepare` 的最终 acceptance、candidate-bound RenderSet/VisualEvidence 与 reference comparison 固定回 canonical observation camera `2cd35f435839f80e364a15a6930ffe37c2145925d6a5b455ccffd30873953736`（canonical `3245ec72…2297af`）。`silhouette_fit_prepare` 仍可保留探索性拟合相机 `04e27c1b…58ad0a`，但该相机只留在 fit evidence，不再进入最终接受/比较。胸壳单 Part 隔离 receipt `docs/evidence/mcp010f/supplemental/part-correction-camera-binding-fix-20260816.json` 为 `PASS_TRANSPORT_WITH_METRICS`，64 evaluations、`accepted + strict_improvement`，acceptance/VisualEvidence camera hash exact-match；比较仍为 `QUALITY_TARGET_NOT_MET`（IoU `0.748146396048`、Boundary F1 `0.350344578024`），candidate 仅 staged、未 confirm/version/export。Stage 0 历史 provisional receipt 仍保留 `BT006_CAMERA_BINDING=FAIL`/`MISMATCH`，不得把该 action-level repair 误写成历史 benchmark 已通过；full current-source Codex replay、Desktop restart、真人/VoiceOver/PBR/export-restart/360 仍未运行或阻断。

2026-08-16 current real Codex ReferenceCanvas/Primary Form Gate：同一源码 cohort `70badfd9d4e07a374aee994ad42604980ccf77e3f572b6d2926b46eccca6f72a` 已真实完成 durable ReferenceCanvas/DesignSpec、canonical Observation 和 Runtime-owned Rig 绑定；authoring mode 为 `RUNTIME_DEFAULT_AUTHORING`。随后 `pelvis → upper-arm-pair` 两步 Primary Form repair 均 64 evaluations、`accepted + strict_improvement`，最终 boundary-only compare IoU `0.752731669182`、Boundary F1 `0.355021780326`、bbox `0.00390625`、centroid `0.001753436389`。这证明 Primary Form 搜索已从 Codex 连续参数搜索移回 Runtime，并且每一步都重新观察/绑定 candidate；但 silhouette gate 仍 `QUALITY_TARGET_NOT_MET`，没有 AOV/typed review/PBR/人评/confirm/version/export 证据。当前只有单一 `perspective` 参考，缺少 `front/back/left/right/rear-three-quarter`，360 继续 `BLOCKED_REFERENCE_COVERAGE`。receipt：`docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260816-primary-form-runtime-fallback-20260816.json`。

2026-08-16 Manifold Boolean current source/raw Gate：当前 vendored Manifold 已通过隔离 Geometry Worker 的真实 C ABI/FFI 执行 bounded same-Part `union`、`difference`、`intersection`；MCP010D raw receipt 为 `16 entries / 16 active`、9 semantic Parts、588 triangles，Boolean curved-mesh lineage 与 residual tangent focused tests 通过。该能力是结构/transport/readback PASS，不是任意 mesh Boolean、视觉/PBR likeness、export/restart、真人或 360 PASS；旧段落中的 unavailable/deferred 结论只保留为历史 cohort。

2026-08-16 CADFit/Render Worker current source Gate：真实 Runtime IPC 已接通 `optimization_job_prepare/get/resume`，Render Worker fit batch 已补齐 `256px` mid fidelity；synthetic raw receipt `docs/evidence/mcp010f/supplemental/optimization-job-raw-20260816-mid-resolution.json` 完成 39 次 `128 → 256 → 512` multi-fidelity evaluation 与 checkpoint/result readback，proposal 无严格改善时返回 `blocked-no-improvement`，不创建 candidate/version。该 Gate 只证明 Runtime-owned 连续参数搜索、严格 Worker 预算和 transport 已落地，不改变真实参考的 `QUALITY_TARGET_NOT_MET`、camera/binding、人评、PBR、export/restart 或 360 状态。

2026-08-16 historical source/async Job correction：该 slice 当时源码 manifest 以 `docs/evidence/mcp010f/source-tool-manifest-summary.json` 为准，为 144 Schema、41 read + 33 opt-in write = 74 tools；`primary_form_repair_job_prepare`/`repair_intent_run_prepare`/`job_get`/`job_result_get` 已在真实 Codex supplemental receipt 中完成 64-evaluation transport。下方 2026-08-15 的 36/23、37/24 或旧 package 数量只保留为历史 cohort 快照，不覆盖当前 manifest。真实 async receipt 仍为 `QUALITY_TARGET_NOT_MET`，不解锁 confirm/version/export、人评、PBR 或 360。

2026-08-15 Primary Form serial composition Gate：真实用户授权 PNG 在同 cohort `78d03f2b…1a808` 的 direct MCP/Runtime/Geometry Worker/Render Worker 隔离运行中完成 `pelvis → upper-arm-pair` 两步 serial repair；每步 64 bounded evaluations，均为 Runtime `accepted + strict_improvement`，并生成 candidate/observation/target/camera/Rig/intent hash-bound 的 `ForgeCADPrimaryFormCompositionLineage@1`。最终比较仍为 `silhouette_iou=0.751411042945`、`boundary_f1_4px=0.34965309767`、`QUALITY_TARGET_NOT_MET`、hard gate false；九 AOV/typed review transport 完成，PBR/人评/confirm/version/export 未运行或未执行，360 为 `BLOCKED_REFERENCE_COVERAGE`。receipt `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-primary-form-sequence-pelvis-upper-arm.json`；Stage 0 benchmark 不变。另修复 probe 在 Codex retry 后错误使用 compact call-id 顺序的 false block，失败诊断保留在 `...-blocked-retry-gate.json`。

2026-08-15 live/package cohort alignment Gate：以当前 `abae43f3` 源码 revision 重建并安装用户级 Dev.app，MCP/Runtime/Geometry Worker/Render Worker cohort `5a1f108a…e2dd2f` 一致；package deep-strict、四资源 allowlist、隔离 Ready/project/preflight probe 通过，包内工具面为 37 read + 24 opt-in write。当前线程已经建立的 Codex MCP 连接仍暴露旧 `7f9e4c…ee518` cohort 和旧 manifest `05fca3…d4d0a`，需要重新建立 MCP 会话后才可执行 live authoring；本 Gate 不绑定任何新 candidate/reference/RenderSet，也不改变 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或人评/PBR/export-restart/360 状态。

2026-08-15 Primary Form resolution-consistent fit Gate：Runtime-owned `silhouette_fit_prepare` 的相机邻域和 geometry trial 已改用 512×512 isolated Render Worker fit batch，与最终 same-camera acceptance 使用同一分辨率；普通 camera-fit 的 128×128 粗搜不变。该 Gate 只消除 Primary Form 的 objective resolution drift，不升级 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 或人评/PBR/export-restart/360 状态。

版本：2026-08-13
状态：`FGC-MCP010A done`；`FGC-MCP010B blocked/deferred（Darwin OS memory hard cap NOT_RUN）`；`FGC-MCP010C source-focused PASS_WITH_UNRUN_VISUAL_GATES`；`FGC-MCP010D source-focused PASS_WITH_DEFERRED_BOOLEAN_AND_VISUAL_GATES（当前 packaged D 结构性探针 PASS，视觉门 NOT_RUN）`；`FGC-MCP010E source-focused PASS_WITH_DEFERRED_EXTERNAL_GATES（当前 packaged E 结构性探针 PASS，但视觉/人评/导出仍 NOT_RUN）`；唯一 `in_progress` 为 `FGC-MCP010F`（Viewer source、packaged CLI read-model、原生窗口与核心控件 smoke PASS；同一 provisional observation 的 packaged Viewer 绑定、正式 VoiceOver、人评和 360 仍 `NOT_RUN/BLOCKED`）。ADR-0026 已新增 Agentic Design Runtime 目标架构；它不改变当前 F 状态。

2026-08-15 Viewer Agentic evidence binding Gate：在 Viewer 已确认 `visualEvidenceBound` 时，Agentic projection 的五类证据 hash 必须逐项等于当前 candidate/reference/render/comparison/quality binding；缺失 hash 不再被当作兼容的 unknown，而是 fail closed 为 unavailable。没有视觉绑定时仍允许结构性 projection 保持 unknown，不把 unknown 升级为质量通过。source Gate、Node behavior、desktop build 与 MCP010F full Gate 通过；质量仍 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，人评/PBR/export-restart/360 未运行或阻断。

2026-08-15 Render Worker fail-closed test boundary Gate：Runtime 的 `render-core` fallback 不再由 `cfg(test)` 隐式参与 fixed/perspective/fit-batch 路径；只有显式 `test-render-worker-fallback` 才允许 legacy 单测回退，生产无 feature 仍只能走 isolated sibling Render Worker。Cargo source ownership checker、无 feature product check、显式 fallback Runtime/MCP tests、MCP010C 与 MCP010F full Gate 通过。该 Gate 只修复证据诚实性与 Worker 落地验证，不升级 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、benchmark incomplete binding 或 human/PBR/export-restart/360 状态。

2026-08-15 Agentic canonical observation MCP dispatch Gate：MCP `InProcess` transport 已与 authenticated Runtime IPC 使用相同的 bound projection 语义；stage/critic/evidence follow-up 缺失或 stale `observation_sha256` 会 fail closed，不能回退到独立观察重建。MCP 全量 56 tests 通过；无新 Schema/tool/CAS 或真实视觉证据，`QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 与人评/PBR/export-restart/360 状态不变。

2026-08-15 Primary Form output-level offset sink：对 mirror/array Part，camera-plane `offset_x/offset_y/offset_z/scale` 必须落在完整 output graph 后的 Runtime-owned typed Transform；源节点只接受 width/height/depth 等局部几何控制。这样 bilateral Part 的整体平移不会被镜像拓扑误解为左右间距变化。该 source/focused Gate 没有新的真实 likeness receipt，视觉状态继续为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`。

2026-08-15 Agentic observation cache source Gate：Runtime 为完整 `AgenticSceneObserveResult@1` 增加 bounded process-local canonical-hash cache；bound plan/critic/evidence/action follow-up 在同一 Runtime 会话消费原观察，cache miss 时重建并严格验证 hash/scope。它不写 Runtime/CAS、不新增 Schema/tool，也不改变 Viewer 质量权威；focused Runtime tests PASS，真实 composition/likeness 仍 `NOT_RUN` 或 `QUALITY_TARGET_NOT_MET`。

2026-08-15 Primary Form composition-lineage source Gate：CLI 现在输出 `ForgeCADPrimaryFormCompositionLineage@1` compact receipt，串行绑定 2–3 个 Part 的 candidate/observation/target/camera/Rig/intent hashes，并在 stale chain、目标漂移、未严格改善或非法 candidate advancement 时 fail closed；第二步起该 prefix projection 会在下一步前实际消费。它只收敛编排与证据消费边界，不是 Runtime durable orchestrator、不是 Render Worker 视觉质量门，也不产生 likeness/high-quality PASS；授权 PNG 缺失时 composition visual Gate 继续 `NOT_RUN/BLOCKED_REFERENCE_BYTES_UNAVAILABLE`。

2026-08-15 Primary Form composition boundary source Gate：CLI 新增 `--part-contour-sequence`，在同一 project 中以 2–3 个 exact Part 为序列；每一步都重新执行 Runtime-bound target/camera/baseline compare/canonical observation/Rig，并只允许一次 `primary_form_repair_prepare`。仅 `prepared` staged candidate 可以推进到下一步，`no_improvement` 必须保留 source candidate 与 source camera；连续参数搜索仍由 Runtime/Geometry Worker/Render Worker 承担。Runtime continuation regression、Runtime full `117 passed / 0 failed / 12 ignored` 和 MCP010C source/raw Gate 通过。Stage 0 Runtime source hash 已同步，checker 通过但仍输出 `camera=MISMATCH`、`QUALITY_TARGET_NOT_MET`、`BLOCKED_INCOMPLETE_BINDING`；没有新的用户授权 PNG，因此 composition visual Gate `NOT_RUN/BLOCKED_REFERENCE_BYTES_UNAVAILABLE`，不产生 confirm/version/export。

2026-08-15 Hip-pair bounded candidate Gate：r21 在 current packaged cohort `726153a3…42ab5c` 完成 60 次 Runtime-owned bounded fit，source/proposal loss `0.426350916959`→`0.426265642648` 严格改善并准备 staged candidate；`PASS_SILHOUETTE_FIT_TO_COMPARE`、九 AOV、strict readback 与无持久用户数据通过。最终 compare 仍为 IoU `0.743473474034`、Boundary F1 `0.301145366407`、bbox `0.0234375`、centroid `0.021831234764`、`QUALITY_TARGET_NOT_MET`、hard gate false；未 confirm/version/export。该 Gate 与 r19/r20 是独立 candidate trial，不是已合并模型，不是 likeness 或高质量 PASS。

2026-08-15 Chest-shell bounded candidate Gate：r20 在 current packaged cohort `726153a3…42ab5c` 完成 61 次 Runtime-owned bounded fit，source/proposal loss `0.426350916959`→`0.416406210214` 严格改善并准备 staged candidate；`PASS_SILHOUETTE_FIT_TO_COMPARE`、九 AOV、strict readback 与无持久用户数据通过。最终 compare 仍为 IoU `0.739507057324`、Boundary F1 `0.314765400977`、bbox `0.013671875`、centroid `0.020515796935`、`QUALITY_TARGET_NOT_MET`、hard gate false；未 confirm/version/export。该 Gate 是 bounded candidate/transport PASS_WITH_QUALITY_TARGET_NOT_MET，不是 likeness 或高质量 PASS。

2026-08-15 Upper-arm-right bounded candidate Gate：r19 在 current packaged cohort `726153a3…42ab5c` 完成 63 次 Runtime-owned bounded fit，source/proposal loss `0.426350916959`→`0.423630597587` 严格改善并准备 staged candidate；`PASS_SILHOUETTE_FIT_TO_COMPARE`、九 AOV、strict readback 与无持久用户数据通过。最终 compare 仍为 IoU `0.744938326895`、Boundary F1 `0.307109823312`、bbox `0.021484375`、centroid `0.021832396893`、`QUALITY_TARGET_NOT_MET`、hard gate false；未 confirm/version/export。该 Gate 是 bounded candidate/transport PASS_WITH_QUALITY_TARGET_NOT_MET，不是 likeness 或高质量 PASS。

2026-08-15 Pelvis bounded repair / camera handoff Gate：r16 的瞬时 `reference_get` 失败与 r17 的错误 camera expectation 均保留为失败证据；CLI probe 已修复 `primary_form_repair_prepare=no_improvement` 时的 source-baseline camera retention，仅在 staged candidate 产生时才切换 repair camera。r18 在 current packaged cohort `726153a3…42ab5c` 通过 canonical observation、26 inferred Part rows、`boundary_error_get=0` 与 `PASS_CAMERA_FIT_TO_COMPARE`；pelvis 55 次 Runtime-owned search 的 source/proposal loss `0.435730135362`→`0.438891599478` 变差，`retained_source`/`no_improvement`，最终 `QUALITY_TARGET_NOT_MET`，无 candidate/version/confirm/export。该 Gate 是编排/证据边界修复，不是 likeness 或高质量 PASS，Stage 0 provisional truth 不提升。

2026-08-15 Canonical observation/Part attribution Gate：Runtime `silhouette_part_error` 允许无显式 Part contour 的 automatic target，并从同一 Render Worker silhouette + Part-ID boundary evidence 生成 26 个 inferred Part rows；CLI 将 baseline compare 放入同一 silhouette turn，在 `scene_observe_get` 之后不再调用 `boundary_error_get`。真实 r15 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-canonical-part-observation-r15.json` 的 cohort `726153a3…42ab5c`、`boundary_error.source=canonical_observation`、`boundary_error_get=0` 通过 transport/observation consolidation；最终 IoU `0.739507057324`、Boundary F1 `0.314765400977`、`QUALITY_TARGET_NOT_MET`、hard gate false，未 confirm/version/export。该 Gate 关闭观察碎片化和 automatic-target unavailable 缺口，不等于 likeness、高质量、Viewer 人评或 360 PASS；Stage 0 provisional truth 保持不变。

2026-08-15 Primary Form right-shoulder bounded trial Gate：同一 current packaged cohort `26354e2f…a2029` 的真实 r11 receipt 对 `shoulder-armor-right` 完成 exact Part contour、63 次 Runtime-owned fit 和 strict same-camera paired acceptance；source/proposal loss `0.426350916959`→`0.409541676181`，只生成 staged candidate。全局 compare 仍为 `silhouette_iou=0.749072115206`、`boundary_f1_4px=0.326405552646`、`QUALITY_TARGET_NOT_MET`，quality hard gate 未通过，故不 confirm/version/export；该 Gate 是 bounded trial/strict acceptance 证据，不是 likeness 或 high-quality PASS，且不替换 Stage 0 provisional truth。

2026-08-15 Primary Form exact-side sink alias Gate：Runtime 对 `*-armor-left/right` 与显式 `*-left/right` detail sink 增加固定、side-aware alias projection，保证 single-Part bounded trial 的 typed width/height/offset 真正进入目标 sink，且不串到另一侧。focused alias/materialization/scope regressions 通过；无新真实 likeness receipt，`QUALITY_TARGET_NOT_MET`、camera mismatch、benchmark incomplete binding、人评/PBR/export-restart/360 状态保持不变。

2026-08-15 Primary Form bounded single-Part trial Gate：既有 `primary_form_repair_prepare`/Job 增加可选 `part_id`，Runtime 将 bilateral Rig 派生为 exact Part Rig，确保只有目标 Part 进入 bounded continuous search；Primary Form 不要求此前已有 Part RenderSet，仍用同一最终 camera、Geometry Worker、strict readback、Render Worker paired compare 和严格 non-regression acceptance。r10 `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-part-contour-trial-r10.json` 在 current cohort `d2b67cd8…02a2f1` 对 `shoulder-armor-left` 返回 `retained_source`、`no_improvement`，source/proposal loss `0.426350916959`/`0.425568832154`，未产生候选；后续 Part proposal 为 `proposal_ready`。顶层 `BLOCKED` 来自无关副作用账本，quality `NOT_RUN`，不提升 `QUALITY_TARGET_NOT_MET`。

2026-08-15 Primary Form single-Part contour/asymmetry Gate：`part_contour_fit_prepare` 现在使用 Runtime 固定 alias 规则解释 bilateral Part-ID：`hip-pair` 聚合左右显式输出，精确 `hip-left`/`shoulder-armor-left` 仍保持单侧 mask、error 与 typed parameter scope；单侧 width/height/offset 控制不污染另一侧。真实 r8 supplemental receipt `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-part-contour-r8.json` 在同 cohort `dd78d216…e5088` 完成 hash→fit 两调用，返回 `proposal_ready`、26 Parts、`PASS_BOUNDARY_ONLY`，无持久化或无关副作用。该 Gate 只证明部件归因和 read-only proposal transport；全局 IoU `0.743548116218`、Boundary F1 `0.300929937747` 仍未达门，quality 为 `NOT_RUN`，不提升 `QUALITY_TARGET_NOT_MET` 或解锁 candidate/version/confirm/export。

2026-08-15 Primary Form 36-control Rig real replay：packaged cohort `106a2889…67b08e` 的 r7 supplemental receipt 完成 56 次 bounded Runtime/Worker fit；hip width/height、hip/pelvis/chest offset 控制已进入 typed Rig materialization，但 source/proposal loss `0.40710507361`/`0.413276643194` 未严格改善，故 `retained_source`、`candidate_state=unchanged`。camera/Render Worker/Viewer read-model 结构绑定通过，最终 IoU `0.746739614479`、boundary F1 `0.34284083431`，仍为 `QUALITY_TARGET_NOT_MET`；该结果只证明有界控制与回退门工作，不是 likeness 或高质量 PASS。

2026-08-15 packaged F 与 QualityReport contract Gate：`AppearancePrepare` 的结构报告将 `visual_status=not-run` 与 `hard_gate_passed=false` 绑定，修复了新的 Runtime validator 暴露的状态漂移；同一 packaged cohort `fee79807…` 的 F raw stdio probe 已 PASS，确认 detail fixture 的 26 parts/4704 triangles、7 material zones、九 AOV、embedded textures、Render Worker same-cohort binding 与 Viewer read-model。该 Gate 只证明结构/运输/读模型，不提升 likeness；视觉质量、人评、PBR likeness、export/restart、360 继续按 `NOT_RUN/BLOCKED` 记录。

2026-08-15 Primary Form 多尺度 geometry convergence：Runtime-owned `GeometryProgram` bounded search 在 64 evaluation hard cap 内优先保留 40 次 geometry exploration，配合 15 次初始 camera neighborhood 与 9 次 geometry-winner refit；coordinate refinement 的 pass scale 固定为 `1.0 → 0.5 → 0.25`，让一次聚合观察支持 coarse-to-fine convergence，而不是让 Codex 进行连续参数搜索。该 source/focused Gate 不改变 Worker/readback/same-camera acceptance 或 Viewer authority，也没有新真实 likeness receipt；当前仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 未运行或阻断。

2026-08-15 camera-fit→compare handoff：Runtime 现在在同一 `project/candidate/target` 作用域复用 `camera_fit_prepare` 的 exact selected camera；未显式传 camera 的 compare 不再回到 default framing。该修复只关闭 source-level camera drift，历史真实 Codex receipt 仍需复跑，当前 `camera=MISMATCH` 与 `QUALITY_TARGET_NOT_MET` 不变。

2026-08-15 Primary Form bounded continuation：Runtime action/job 在不扩大 64-evaluation hard ceiling 的前提下最多执行两轮 camera/geometry continuation；第二轮围绕第一轮 incumbent 继续局部收敛，Codex 仍只提交一次 typed intent。focused fixture 已验证 `iterations=2` 与 63–64 evaluations；没有新的真实 likeness receipt，当前 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH` 和 benchmark incomplete binding 不变。

2026-08-15 Primary Form same-camera retention：Runtime 在 `geometry_prepare` 前用最终选定 camera、同一 reference target、同一 512px Render Worker 和同一 weighted loss 对 authored source 与 proposed GeometryProgram 做不持久化 compare；仅当 proposal 严格优于 source 才创建 staged candidate。`PrimaryFormAcceptance@1` 绑定两侧 loss、camera/program hash 与 `accepted`/`retained_source`；失败时不覆盖 source VisualEvidence。该模块通过 contracts/Runtime focused regression，仍不提供新的真实 likeness 证据，保留 `QUALITY_TARGET_NOT_MET`、`MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 与 human/PBR/export-restart/360 未运行状态。

2026-08-15 Primary Form action budget module：修复真实 current-cohort 发现的 action-level truncation——detail 请求声明 64，但旧 Runtime cap 只允许 24。当前 Runtime cap 恢复到固定 64，且由 `normalize_primary_form_repair_optimizer` 统一 clamp；`primary_form_repair_prepare` focused fixture 实际消耗 63 次，说明连续参数搜索仍在 Runtime/Worker 内部。新 cohort Dev.app 已安装，但修复后 Codex authoring/hash/prepare 复跑阻断，未产生新的 likeness/compare receipt；因此本计划只记录收敛能力修复，保留 `QUALITY_TARGET_NOT_MET`、`MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 与 human/PBR/export-restart/360 未运行状态。

2026-08-15 packaged Render Worker landing：同 cohort Dev.app 的 Resource allowlist 与 ad-hoc deep-strict package verify 已确认 `forgecad-render-worker` 与 MCP/Runtime/Geometry Worker 同包；packaged Runtime 隔离 raw probe 完成九 AOV、固定 512px renderer、两次 deterministic hash、candidate-bound compare 和 MCP image-block transport。证据为 `docs/evidence/mcp010f/dev-app-install-render-worker-20260815.json`、`dev-app-package-verify-render-worker-20260815.json`、`packaged-render-worker-raw-20260815.json`。该 receipt 使用 synthetic reference，只证明 Render Worker 的 packaged 落地和 typed transport，不证明机器人 likeness/PBR/人评；质量仍 `QUALITY_TARGET_NOT_MET`，360 仍 `BLOCKED_REFERENCE_COVERAGE`。

2026-08-14 Primary Form 联合 proposal 回退：在 dominant Part 的 evidence-attributed width/height/offset 联合提案过冲时，Runtime 固定尝试 `1.0 → 0.5 → 0.25` authored-baseline interpolation，之后才执行单坐标探测；严格改善晋级、总预算、Geometry/Render Worker 和 Viewer 只读边界不变。focused/full Runtime 与 contracts 通过；无新的授权参考字节或视觉 receipt，当前仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`。

2026-08-14 Primary Form bilateral Part-ID projection：Runtime boundary proposal 现在把 Render Worker 的显式左右 Part-ID 合并到 Rig 的 `*-pair` semantic Part，再计算局部 envelope 的 width/height/offset proposal；这修复了 dominant pair 已命中但 pair proposal 仍为 authored baseline 的收敛断点。pair focused/full Runtime、MCP010C/F source 与 Render Worker boundary Gate 通过；没有新的授权参考字节或视觉 receipt，质量与 benchmark 真值保持不变。

2026-08-14 Primary Form 单 Part repair scope：Runtime 现在把主导 candidate-bound Part-ID boundary error 设为一次 bounded repair 的唯一 mutable scope，其他 Part 的 typed proposal 恢复 authored baseline；`DesignCriticReport@1` 的 Codex-facing repair operation 直接指向 `primary_form_repair_prepare`，避免把 `silhouette_fit_prepare` 重新暴露成连续参数搜索。Runtime/Agentic contract/MCP010C/F/Render Worker boundary Gate 通过；这仍只是收敛与编排修复，没有新的授权参考字节或视觉 receipt，现有质量与 benchmark 阻断不变。

2026-08-14 Primary Form Part-priority follow-up：在 Runtime 已产生 candidate-bound Part-ID boundary segments 后，bounded geometry probe 排序优先于主导 Part 的聚合 contour distance，再以 typed proposal delta 和稳定参数 ID 排序；没有 Part-ID evidence 时沿用旧 fallback。此模块修复只改善有限预算的误差覆盖，不是 likeness 门；本轮真实复验因授权参考原图字节不可用而 `BLOCKED_REFERENCE_BYTES_NOT_AVAILABLE`，当前质量与 benchmark 真值不变。
依赖：`FGC-MCP009 done（MVP host golden path）`

当前账本校正：源码合同为 191 个 JSON Schema、21/21 active operators，工具面为 54 个默认只读工具和 36 个显式 opt-in write 工具（共 90）。新增 Mechanical Animation Clip 的 prepare/get/preview 只形成 Runtime-owned immutable rigid-action sidecar 与同 cohort Worker replay，不提供角色动画或任意脚本；既有 `FictionalEnergyRifleProfile@1`/`FictionalEnergyRiflePlan@1` 仍仅作 nonfunctional source authoring aid；`job_result_get` 与 `primary_form_repair_job_prepare` 继续把 bounded fit→typed GeometryProgram→strict readback→Render Worker→compare 收口为 Runtime-owned staged prepare，不 confirm/version/export。Agentic projection 与 durable session/checkpoint/RepairIntent prepare/readback 另有独立 receipt；它们仍是结构/编排能力，不是 likeness 通过。

2026-08-14 Primary Form 首轮全控制覆盖修复：26-control detail Rig 的 CLI fit budget 由 32 提升为 64；Runtime 在 GeometryProgram 路径固定执行 `32 geometry + 16 initial-camera + 16 geometry-winner-camera-refit`，几何阶段先跑一次证据提案，再逐一覆盖 26 个控制，剩余几何预算才进入反向方向。该路径仍是 Runtime-owned bounded typed search，Codex 不接收或驱动连续参数轨迹。26-control focused/full Runtime、MCP010C/F source Gate 通过；没有新的真实机器人视觉 receipt，`QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING` 和 human/PBR/export-restart/360 未运行状态保持不变。

2026-08-14 Primary Form 肢段尺度/装配控制扩展：detail probe 在现有 `SilhouetteRig@1` typed `height` 与 `offset_y` 语义上，从 20 个控制扩展为 26 个，新增 upper-arm/forearm/thigh/shin height 与 elbow/knee vertical placement。Runtime 继续以 DAG-aware materialization 和有界确定性 schedule 承担参数试算，Codex 只提交一次 typed Rig；没有新的授权机器人视觉 receipt。focused/full Runtime、MCP010C 与干净 worktree 的 MCP010F source Gate 通过，当前机器真值仍为 `QUALITY_TARGET_NOT_MET`、camera `MISMATCH`、`BLOCKED_INCOMPLETE_BINDING`，human/PBR/export-restart/360 仍 `NOT_RUN/BLOCKED`。

Stage 0 机器真值唯一入口为 `docs/evidence/mcp010f/current-benchmark-truth.json`。attempt35 只是 provisional retained observation，不是已成立 benchmark：它是 `QUALITY_TARGET_NOT_MET + INCOMPLETE_TRUTH_BINDING`，benchmark eligibility 为 `BLOCKED_INCOMPLETE_BINDING`，fit/compare camera 为 `MISMATCH`；packaged Viewer 的 current-cohort read-model binding 已单独通过，但不等于 attempt35 的 packaged visual E2E。当前 r3 只证明 Runtime-owned single-action transport，仍不能越过 PBR likeness、独立真人、export/restart 或 360 门。

2026-08-14 Agentic visual evidence consolidation：target-bound `SilhouetteTarget@1` is now carried as nullable `target_sha256` in Runtime visual evidence and read-only Viewer/Agentic lineage. `DesignCriticReport@1` exposes a fixed-priority `primary_form_directive` and one Runtime-owned bounded RepairIntent for Primary Form failures；Codex receives one coherent action context and does not search continuous parameters. Target-bound Runtime round-trip, full Runtime/Store tests, contract checkers and `script/test_mcp010f.sh` passed. This is evidence/context infrastructure only：Viewer still consumes Runtime quality truth, Repair is not executed, and real visual quality remains `QUALITY_TARGET_NOT_MET` with `MISMATCH`/`BLOCKED_INCOMPLETE_BINDING`.

2026-08-14 Viewer evidence lineage follow-up：Runtime `visual_evidence` now validates the complete candidate artifact → RenderSet → comparison → QualityReport chain together with reference/target/camera binding before exposing `ViewerVisualEvidence@1`; missing comparison and cross-artifact relinking fail closed in Runtime, so the Viewer no longer carries the sole responsibility for reconstructing this boundary. This is a source/runtime integrity repair only; it does not change the current robot visual result or unlock human/PBR/export/restart/360 gates.

2026-08-14 Primary Form single-action transport follow-up：current-cohort receipt `real-codex-cli-current-20260814-primary-form-runtime-owned-r3.json` verifies one coherent Runtime-owned action after aggregated observation: `primary_form_repair_prepare` performs bounded fit → Geometry Worker → strict readback → Render Worker → compare, and the CLI consumes its candidate-bound visual evidence instead of issuing a second compare. This closes the duplicate search/compare orchestration gap and records camera binding `PASS_SILHOUETTE_FIT_TO_COMPARE`; metrics remain below the visual gate (`IoU 0.749122`, `Boundary F1 0.347623`), so it is not a high-quality or benchmark PASS.

2026-08-15 Primary Form local-group convergence follow-up：when the Runtime joint evidence proposal is rejected, the bounded geometry phase now tests Part-local coupled width/height/offset hypotheses before scalar coordinate probes. This preserves the Runtime/Worker ownership boundary and does not expose a continuous search trace to Codex; focused tests pass, but no new real reference receipt has been generated and the previous visual truth remains below gate.

<!-- forgecad-stage0: schemas=402 schema_set_sha256=2f31c744134257e4d455cfc801a9be9e5c38ae81b1bc7fabd80d2002f968c4c7 read_tools=90 write_tools=69 total_tools=159 task=FGC-MCP010F observation=QUALITY_TARGET_NOT_MET eligibility=BLOCKED_INCOMPLETE_BINDING evidence=INCOMPLETE_TRUTH_BINDING camera=MISMATCH packaged=PASS_CURRENT_COHORT_BOUND_READ_MODEL latest_attempt=real-codex-cli-current-20260815-b37-complete-auto-v3.json latest_completed=real-codex-cli-current-20260815-b37-complete-auto-v3.json -->

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

MCP006 的 44-contract、MCP010B 已保存的 50/52-contract aggregate，以及 3c/f488/bfa56/d9 Dev.app/raw/CLI receipt 都是历史或结构事实，原样保留。该历史段当时源码总计 **144 个 JSON Schema**：历史合同、MCP010B/C/D/E/F 当前合同、Agentic contract family、`RuntimeJobResult@1`、`RepairIntentRun@1` request/result、weapon joint-multiview V2 contracts 与 Fictional Energy Rifle Profile/Plan contracts。当前 source Gate 已通过 B 的 V2 geometry/readback/Worker isolation、C 的 fixed renderer/九 AOV/reference compare/review raw path、D 的真实 Operator、E 的离线 AssetPack/UV/PBR/MikkTSpace raw path，以及 F 的哈希绑定轮廓目标、扩展相机搜索、Runtime-owned camera reference、受限 Rig/SDF fit、Runtime-owned bounded Primary Form search、单/多 Part contour proposal、Part error table、candidate compare、异步 `primary_form_repair_job_prepare`/`job_get`/`job_result_get` staged prepare/evaluate path 和 weapon joint-multiview source slice；Agentic projection 与 durable prepare/readback 也通过合同/Viewer/隔离重启 probe；当前工具面为 46 read + 33 opt-in write。C/E/F receipt 使用 synthetic 或结构性 reference，只证明绑定、传输、持久化和 deterministic bytes，不证明 PBR likeness、用户 robot likeness、Viewer/package/live、人评或 360。Agentic durable receipt 与本轮 joint source Gate、Profile source Gate 也不证明通用单动作 orchestrator、Repair execution 或视觉 PASS。下文任何未特别注明的旧“50/52/65-contract/current Dev.app”叙述都应按本段分层。

| 项目 | 当前事实 | MCP010 目标 |
| 说明 | `5143ac3b…6e61`、bfa56 与 d9c23b…ac0bd 仅是历史 package/live receipts；当前源码 revision `aade327c` 的最新 Dev.app 安装 cohort 为 `6f00a58a…ed50`（receipt `docs/evidence/mcp010f/dev-app-install-pdk-v0-20260817.json`），隔离 package probe PASS，Codex Desktop 完整重启 Gate 仍 `NOT_RUN` | live 证据仍只证明结构工具链，不提前写成视觉/PBR能力 |
|---|---|---|
| 合同 | MCP006 历史为 44 个 JSON Schema；当前 MCP010B/C/D/E/F source contracts、Agentic contract family、`RuntimeJobResult@1`、`RepairIntentRun@1`、weapon joint-multiview V2 contracts、Fictional Energy Rifle Profile/Plan、Parametric Group v2、Subdivision crease/root-lineage 与 Authoring Mesh contracts 使当前 manifest 为 191 个 JSON Schema（含 `CameraCalibrationRef@1`、`CameraCalibration@2`、`CameraRigCalibration@1`、`SubjectCoordinateFrame@1`、`OptimizationIntent@2`、`OptimizationEvaluation@2`、`OptimizationResult@2`） | 维持版本化合同；后续任务只可新增有证据的 Viewer/闭环合同 |
| MCP | 该历史段源码为 41 read + 33 opt-in write（74）；F 新增 Runtime-owned `silhouette_rig_hash` 以避免 Codex 本地重算 Rig canonical hash、`silhouette_part_error_get` 多 Part 误差表和 `primary_form_repair_prepare` 单动作 staged prepare/evaluate 与 `repair_intent_run_prepare` CAS-bound staged run；Agentic 新增 projection tools、session/checkpoint readback 和 approval-gated prepare tools；C/E/F/Agentic source raw/restart Gate 已按各自范围通过，历史 Dev.app receipts仍按 cohort 保存 | 真实用户 likeness、同一 candidate 的 packaged Viewer、人评/PBR/360证据仍需独立 Gate；不得用 synthetic/raw、prepare receipt 或 projection 直接宣传高质量 |
| Skill | 十个历史 first-party `0.1.0` declarative Bundle + 当前 `primitive-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`uv-pbr@0.2.0` active overlay；AssetPack 独立于 Skill | 仅在真实 consumer、bundle integrity、AssetPack provenance 和 benchmark 都通过后保持 active |
| 几何 | MCP010D source 已提供 primitive、profile/extrude、loft、revolve、tube-sweep、transform、mirror、array、panel、vent-array、joint-stack、part-output 与 bounded same-Part Boolean；同 cohort packaged D raw structural probe 已通过 | Boolean 仍仅限受约束的同 Part union/difference/intersection；真实用户视觉阈值仍未运行 |
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

当前进度：010A/010B 历史与 source structural evidence 见 `docs/evidence/mcp010a/`、`docs/evidence/mcp010b/`；C source evidence 见 `docs/evidence/mcp010c/`；D/E/F source evidence 见 `docs/evidence/mcp010d/`、`docs/evidence/mcp010e/`、`docs/evidence/mcp010f/`。该历史段源码总计 144 个 JSON Schema、41 read + 33 opt-in write = 74 个工具。用户第一次 Desktop restart 的 FAIL receipt和后续结构 PASS receipt均保持原样。C 当前源码已在隔离 raw stdio 中证明 56-tool source manifest、九 AOV、candidate-bound comparison、MCP image block、Codex typed visual review 与 deterministic bytes；E raw stdio 已证明 AssetPack manifest/provenance、embedded PNG textures、512px UV atlas、固定 mikktspace、PBR bindings 和同一九 AOV render path；F source Gate 已新增 hash-bound silhouette target、扩展 camera search、`CameraCalibrationRef@1`、Runtime-owned bounded Primary Form/SilhouetteRig/SDF fit、单 Part contour proposal、candidate compare、weapon joint-multiview source slice、只读 Viewer 的九 AOV、reference/render split/overlay/flicker、Part/MaterialZone 筛选、爆炸图和热图辅助及 TypeScript/Vite/Tauri 构建，另有 packaged CLI read-model、原生窗口与核心控件 smoke；Agentic projection、durable prepare/readback 与 CAS-bound RepairIntentRun 另通过 preflight 顺序、空 reference fail closed、合同 checker 和 Runtime/MCP 重启 probe。C/D/E/F 结构/传输证据不是用户机器人 likeness；同一 provisional observation 的 packaged Viewer 绑定、正式 VoiceOver、独立人评阈值、xatlas/Validator、真实 PBR likeness、export/restart hash 和 360仍 `NOT_RUN/BLOCKED`。短时 launcher flock 只用于启动选主，Runtime `runtime.writer.lock` 才是最终唯一写者。

### 3.2 FGC-MCP010B — V2 合同与几何真值

兼容界限：`[transition-v1]` `GeometryProgram@1` 继续服务已存在的 MCP007–009 primitive-only appearance/export MVP 路径，且 Runtime 现会对其 GLB 作物理回读；它不是 MCP010B 的 V2 high-quality 写路径，也不得借此获得 V2 catalog、strict `ArtifactReadback@2`、九 AOV strict compare 或材质声明。历史对象不迁移、不改写。V1 新写入口的最终移除须与 MCP010E 的 `AppearanceProgram@2` 迁移一并设计，不能在 B 中让当前已验收的 V1 appearance/restore/export 链静默断裂。

Owned：`GeometryProgram@2`、`OperatorCatalog@1`、`ArtifactReadback@2`、GLB/accessor validator、primitive 修复及负向 fixture。

当前 B source Gate 与 C/D/E/F source Gate 已通过：B 覆盖 V2 geometry/readback/Worker isolation/restore；C 覆盖当前合同 checker、固定 renderer、九 AOV、local mask/metrics、candidate-bound review、MCP image block 和 deterministic raw stdio（C 历史 subtotal 为 59）；D 覆盖当前 19-entry active source catalog（含 bounded Boolean）；E 覆盖离线 AssetPack、512px UV atlas、fixed mikktspace、embedded PBR/九 AOV；F 覆盖哈希绑定轮廓目标、37 个覆盖全局尺度的粗候选加 9 个局部探针、扩展 Rig/SDF 搜索、单 Part proposal、2–8 候选比较、方向性边界误差、只读 Viewer 的 AOV/compare/Part/MaterialZone/explosion/heatmap source surface、TypeScript/Vite/Tauri 构建和 write-boundary negative check。Agentic projection、durable session/checkpoint/RepairIntent prepare/readback 与 CAS-bound RepairIntentRun 另有独立合同/重启 Gate。该历史段 source tool manifest 为 41 read + 33 opt-in write = 74。历史 package/CLI/live receipts保持原样；C/D/E/F/Agentic synthetic/raw/source不等于真实 robot likeness、PBR likeness、同一 candidate 的 packaged Viewer、人评或 360。Darwin OS total-memory hard cap、xatlas、Khronos Validator仍 `NOT_RUN`；授权单图仍只能产生 `PARTIAL_VISIBLE_VIEW_PASS`，HQ_360 仍 `BLOCKED_REFERENCE_COVERAGE`。

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

当前源码工具数量为 54 read + 36 opt-in write = 90。MCP010A/010B 的 30/32-tool Dev.app receipts均为历史 structural cohort；当前源码 Dev.app 的同 cohort install/probe 已通过，Codex Desktop 完整重启 Gate 仍 `NOT_RUN`。C source raw 已证明 `render_pass_get` image block 和三项视觉证据工具，D 当前 packaged raw 已证明同 cohort Operator/strict readback transport，E raw 及当前 packaged E 已证明 `material_pack_get`、embedded texture 和九 AOV render path，F source 已证明轮廓目标、37 个覆盖全局尺度的粗候选加局部探针相机拟合、`CameraCalibrationRef@1`、Rig/SDF/Part/candidate compare、边界误差读取、Subdivision artifact-lineage read projection、Authoring Mesh source operator、`authoring_mesh_edit_prepare` approval-gated candidate staging、`primary_form_repair_prepare` staged prepare/evaluate 与 `repair_intent_run_prepare` CAS-bound staged run；Agentic projection 与 durable prepare/readback 已通过合同 checker、preflight 顺序、空 reference fail closed 和 Runtime/MCP 重启 probe。packaged Viewer 已有 read-model/window/core-control smoke，但与 attempt35 provisional observation 的 package binding、正式 VoiceOver、真实用户 likeness/PBR likeness、人评阈值和所有 360° evidence仍 `NOT_RUN/BLOCKED`。

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
- `boolean@1`：只允许同一 Part scope 的 union/difference/intersection；
- `panel@1`、`vent-array@1`、`joint-stack@1`；
- `part-output@1`：一个语义 Part 可由多个细节节点组成。

Manifold 固定目标为 v3.5.2，仅 C API 静态进入隔离 geometry worker，关闭 Python/JS binding、自动下载和不受控并行。采用前必须有 full revision、LICENSE hash、transitive SBOM、恶意输入/时间/内存/确定性/source-ID benchmark 和 removal plan；在 receipt `accepted` 后，当前只开放 bounded same-Part union/difference/intersection，绝不扩展为任意 mesh Boolean。

当前实现结果：`operator_catalog_get` 返回 19 项且均为 `active`（含 `boolean@1` 的 bounded same-Part union/difference/intersection）；`script/test_mcp010d.sh` 已通过 contracts、source-built Worker/Runtime/MCP、raw stdio `catalog → hash → prepare → readback`、Boolean curved-mesh lineage、strict readback、determinism、future-input/unknown-parameter/Boolean negative。`hard-surface-detail@0.2.0` 的 manifest、recipe、operator lock、benchmark fixture、LICENSE/NOTICE、SPDX SBOM、provenance 和 development trust 均通过 Runtime integrity 后才返回 `active`。证据：`docs/evidence/mcp010d/manifest.json`、`focused-gates.json`、`docs/evidence/mcp010d/raw-stdio-subd-cage-20260815.json`。

当前 Manifold adoption receipt 已为 `accepted`，因此 bounded Boolean 的结构/transport/readback Gate 已通过；这不证明 Viewer presentation、真实 Codex Desktop D 或视觉 likeness。完整恶意输入/内存/确定性/source-ID 及第三方 Validator 的分发级 Gate、真实 PBR likeness/纹理审美和 360°仍继续记录为 `NOT_RUN/BLOCKED`。

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

2026-08-15 真实 Primary Form/Render Worker 复放：同 cohort `78d03f2b…1a808` 的 Dev.app direct MCP 隔离复放使用 CAS 中的用户授权 PNG，完成 `detail → silhouette-first → primary_form_repair_prepare → nine AOV → visual_review → quality_get`。receipt `docs/evidence/mcp010f/supplemental/real-codex-cli-current-20260815-primary-form-runtime-replay.json` 记录 64 次 Runtime-owned bounded evaluation、26 Parts、4704 triangles，source→staged candidate strict improvement；最终 `boundary_f1_4px=0.414236535223`、`silhouette_iou=0.74637831219`、`bbox_edge_error=0.03515625`、`centroid_error=0.007345230528`，仍 `QUALITY_TARGET_NOT_MET`、`quality_hard_gate_passed=false`、`visual_review=needs_revision`。这验证了 Primary Form 连续参数搜索不再由 Codex 承担、Render Worker 走同 cohort 真实执行路径；不解锁材质/PBR、confirm/version/export 或 HQ 360，Desktop live MCP 旧会话重绑仍需后续完成。

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
