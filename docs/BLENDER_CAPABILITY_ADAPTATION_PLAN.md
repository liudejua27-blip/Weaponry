# Blender 能力研究与 ForgeCAD 适配计划

> **Status: reference-only historical document (2026-08-29).** 本文保留研究或审计发生时的事实，不再定义当前产品范围或任务顺序。可复用结论必须经过 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md`、ADR-0029 和当前真实证据重新验证后，才能进入穿越火线武器主线。

> 2026-08-26 现行 source 为 **525 schemas / 112 read + 84 write = 196 tools**。Native High/Material plan 仍由 ForgeCAD typed Worker 负责，不把 Blender/DCC 状态设为产品真值；真实 D1 仍阻断于 authored orientation。

> 2026-08-25 商业质量收口：本文件只保留 clean-room 概念研究。ForgeCAD 商业武器主链由自有 AuthoringMesh/High/Low/UV/Cage/Bake/Surface Workers 执行；Blender、`bpy`、`.blend`、BlenderMCP 或任意脚本不是产品依赖、fallback 或质量真值。新的原生落地顺序见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。

> 2026-08-26 现行 source 口径为 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools**；下文 195/24/90 等数字只属于历史 clean-room slices。Blender 永久 `reference-only / unavailable-for-product`，不会晋级为 package Worker。

版本：2026-08-23
状态：`FGC-MCP010F in_progress`；Modifier Stack、TopologySnapshot、bounded Bevel/Normal、Modifier evaluation v2、bounded Subdivision evaluation v2、fixed RenderProfile/AOV lineage、Mechanical pose/sequence/transient geometry preview、Parametric Group v2、Render Evidence Integrity、Render Evidence Replay、Boolean Operand Lineage、bounded crease-aware Subdivision、Subdivision root-lineage、artifact-lineage reconstructed projection/durable sidecar 与 product-owned Authoring Mesh source/focused slices 已实现，完整目标未完成。

## 1. 结论

ForgeCAD 不安装、不调用、不嵌入、不捆绑 Blender，也不提供 Blender Worker、Blender fallback 或 `.blend` 导入真值；Blender 的 `.blend`、DNA/RNA、BMesh、Depsgraph、EEVEE、`bpy` 和任意 Python 都不能进入产品执行链。本计划只从 Blender 学习数据分层、非破坏求值、渲染 pass、颜色语义、High-to-Low/Cage Bake 问题定义和动画求值边界，再以 ForgeCAD 自有 closed Schema、Rust Runtime、typed Worker 协议和 canonical hash clean-room 实现。FPS 武器生产的权威目标和阶段门见 ADR-0027。

本轮官方研究冻结到 `blender/blender` commit `72ccdd6e96ca119a1ffa3372559cc5654343b477`（2026-08-18）。未克隆、安装、编译或执行 Blender；未复制 Blender 源文件。Blender 根 `COPYING` 将项目置于 GNU GPL，故官方源码只作为 reference-only 架构研究材料；冻结许可证、Modifier/Depsgraph header blob 与拒绝能力已记录在 `docs/evidence/adoption/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477.yaml`。`intern/cycles` 中的个别 Apache-2.0 文件也不能让整个 Blender 或完整 Cycles 自动成为可采用依赖；任何未来采用必须逐文件、逐依赖重新审计。

## 2. 当前能力与真实缺口

| Blender 方向 | ForgeCAD 当前真实能力 | 缺口与处理 |
| --- | --- | --- |
| Mesh/BMesh | `GeometryProgram@2`、24 个 product-owned typed operator；`panel@2` 生成 bounded recess/border/bevel/support-loop 矩形 Part；`vent-array@2` 生成 connected/watertight 贯穿开槽面壳与同一 Part 内 backing sub-solid；`recessed-channel@1` 生成 bounded standalone 变宽变深 channel Part；`authoring-mesh@1` 显式 V/E/Loop/triangle-quad Face stable root IDs；candidate-bound `AuthoringTopology@1`、translate/boundary-face extrude preview，以及 approval-gated `authoring_mesh_edit_prepare` reviewable candidate staging；严格 GLB readback与 bounded `TopologySnapshot@1` | 已有受限 source/read/preview/prepare 链，但 `panel@2` 不是任意宿主 mesh inset，`vent-array@2` 的 backing 也不是独立 semantic Part，`recessed-channel@1` 也不是任意宿主切槽；仍没有 selection/history、任意 BMesh 操作、跨 lineage element ID、通用 mesh editor 或 Python 扩展 |
| Modifier/Depsgraph | 有序 typed operator DAG、非破坏 Modifier Stack lowering、v2 semantic evaluation/dirty/cache-reuse decision，以及 candidate-bound `GeometryModifierApplyRequest@1` 的 approval-gated reviewable staging | 当前 Apply 只允许一个 stable Part 的唯一 direct source，支持 transform/mirror/array 与 bounded direct-box bevel/corner normal；没有外部 Part/time/mode dependency graph、通用持久 mesh cache、任意 modifier 或 Blender Depsgraph parity |
| Subdivision | `subd-cage@1` 保留无 crease 兼容面；`subd-cage@2` 实际执行 3..16 规则开放 quad cage、levels 1..2、整数 edge sharpness 1..2 与 level decay；preview 返回完整 root mapping，artifact projection 以 durable evidence + strict readback + full-GLB byte replay 绑定唯一 direct source primitive 的 local triangle ranges | 不是 arbitrary topology/OpenSubdiv/limit surface；无 fractional/vertex crease、adaptive、face-varying UV、per-level child path、corner domain、influence weight、persisted sidecar、glTF vertex/edge/corner identity 或跨版本 ID；Modifier Stack v1 未接纳 @2 |
| Boolean | 隔离 Manifold Worker 的 same-Part 双输入 union/difference/intersection | 不扩展成 Blender collection/多 solver 语义；先补 topology/lineage 投影 |
| Bevel/Normal | active `bevel@1` 只处理 direct primitive box 全部 12 edge；`normal-policy@1` 为 corner-domain face-area×face-angle + crease | 已保留 source box lineage 并拒绝 normal-policy 之后再做 Boolean；不支持 explicit semantic edge、任意 mesh、miter/material/face-strength 或通用 Weighted Normal parity |
| EEVEE/Cycles | 固定 512×512 software renderer、固定相机、九 AOV、PBR 通道；`RenderProfile@1` 已绑定 engine/backend/sampling/color/AOV/palette hashes | 没有 GPU 实时引擎、路径追踪、Shader Graph、OCIO、EXR 或 custom AOV；Blender/Cycles/EEVEE 仍只作分层参考 |
| Animation/Rig | 独立 `MechanicalRestFrame@1`、有限 `MechanicalPoseAction@1`、Runtime-owned immutable clip、候选绑定的层级姿态/双 Worker transient GLB 求值，以及 Viewer 对 scheduled single tick 的 verified rigid Part 投影；`SilhouetteRig` 继续只服务轮廓优化 | rest frame 仍是 caller-authored，未证明原资产 rig/pivot/domain；只有显式离散 tick、无自动连续播放；没有 armature、skin、IK/NLA/F-Curve、约束图、可编辑时间轴或 glTF animation channel |
| Geometry Nodes / plugin ecology | 三个 immutable first-party `ParametricDesignKitRequest@2` group template，绑定 typed socket、template/catalog/instance/program/source-map hash | 不是 node editor/runtime；没有 nested group、field/attribute/simulation、custom node、dynamic plugin 或 marketplace |
| Python ecosystem | MCP/Runtime typed command surface | 永久拒绝 `python.exec`、脚本、路径、URL、环境变量；只学习稳定数据/命令 API |

## 3. 已实现的原子切片

`MechanicalPoseGeometryPreviewRequest@1 → MechanicalPoseGeometryPreview@1` 与默认只读 `mechanical_pose_geometry_preview` 把既有 rigid pose 从 TRS projection 收紧到可编译的 transient geometry：Runtime 重验 durable candidate/artifact/readback/program/catalog/config cohort，按每个 Part 的 `PoseWorld × inverse(RestWorld)` 生成 delta，并只在 final Part output 可递归证明为唯一纯 Part source 时追加 bounded `transform@2` sink；派生 program 重新 hash，由 fixed Geometry Worker 编译并做 strict GLB readback。结果只交付 program、artifact hash 与 readback metadata，CAS/SQLite/candidate/version 不变；Quaternion lowering 在 Euler gimbal 附近 fail closed。该能力仍是 caller-authored rig structural preview，不是原资产 Armature/skin/animation 或 Blender parity。

`SubdivisionArtifactLineageRequest@1 → SubdivisionArtifactLineageProjection@1` 与默认只读 `subdivision_artifact_lineage_get` 把上一 slice 的 program-bound root lineage 收紧到真实 candidate artifact：

- 调用方只提交 exact project/candidate/artifact/readback/node/budget/canonical；Runtime 从 durable `GeometryCandidateEvidence@1` 和 CAS 读取 persisted program、readback object、catalog/config hashes，不信任调用方提交 program 或 bytes；
- Runtime 复用 confirmation 的 strict V2 evidence/readback Gate，再让 fixed Geometry Worker 对 persisted program 重编译，要求 replay GLB SHA 和全部 bytes 与 CAS artifact 完全相同；
- 只有 target `subd-cage@2` 在 `ArtifactReadback@2.part_bindings` 中是唯一 direct source primitive、open surface 且 triangle count 与 evaluated topology 相等时，才输出 control-quad local triangle ranges 和固定 `quad q → triangles 2q,2q+1`；
- focused Runtime 覆盖 no-write、restart、stale readback 与重哈希错误 range；MCP 覆盖 closed tool、summary/structuredContent 和完整 wire 1 MiB；实际 artifact-bound 大样例为 14×14 level2、5,408 triangles、17,162 lineage elements，并在 1 MiB 下保留至少 4 KiB 余量；
- 结果明确是 `read-only-reconstructed-projection-not-persisted-sidecar`，不提供 glTF vertex/edge/corner identity、cross-version ID 或视觉质量。

当前源面是 195 schemas、24/24 active operators、54 read + 36 opt-in write = 90 tools。`recessed-channel@1` 已加入 bounded standalone 变宽变深 channel grammar，但不等于任意宿主 mesh cut。Authoring Mesh source slice 仍以显式 V/E/Loop/F、确定性 fixed-Worker GLB 与 strict readback 验证；`AuthoringTopology@1`/`AuthoringMeshEditPreview@1` 提供 candidate-bound source topology 与 transient translate/boundary single-face extrude，`authoring_mesh_edit_prepare` 只写 approval-gated reviewable candidate。`geometry_prepare` 的 exact branches、candidate-bound Modifier Apply、Render Evidence Replay、Mechanical Animation Clip 与 Subdivision lineage 继续保持各自 structural evidence；它们都不是 persistent editor、任意 BMesh/Python/plugin、Blender parity 或视觉质量门。glTF V/E/C identity、跨版本 element ID、package/live/render/visual/human/PBR/export-restart/HQ360 未完成。

Modifier Apply 的准确边界是：它读取并重验一个 durable current-head candidate，但不会就地修改它；输出是另一个 approval-gated、reviewable candidate 和 immutable `GeometryModifierApplyResult@1` CAS sidecar。证据证明同进程 same-key single-flight、30 秒 bounded waiter、CAS reservation、source/derived 双 Worker replay、目标/非目标 Part binding 保持与 Runtime/MCP 重启读回；Store 最终事务会再次读取 source/derived program、readback 和 sidecar 做独立复核。它不证明跨进程 single-flight、任意拓扑 Part 修改、通用 Depsgraph、视觉改善、用户批准或永久版本创建。

`SubdivisionTopologyLineageRequest@1 → SubdivisionTopologyLineage@1` 与默认只读 `subdivision_topology_lineage_preview` 提供控制笼根元素到最终评估四边形拓扑的有界投影：

- 固定 Geometry Worker 复用 `subd-cage@2` 的真实求值器，而不是在 Runtime/MCP 伪造拓扑；control vertex 映射 retained evaluated vertex，control edge 映射最终 evaluated edge chain，control quad 映射连续 evaluated quad/triangle range，显式 crease 另绑定对应 edge chain；
- evaluated edge ID 是最终四边形拓扑中按端点字典序编号的无向边，明确不是 glTF triangle、去重 vertex 或跨 program 稳定 ID；固定 triangulation 为 `0-1-2_0-2-3`；
- Runtime 独立重算 program/catalog/node、规则网格 V/E/Q/T 计数、control edge/quad 覆盖与 crease chain，并重验 lineage/canonical hash；请求预算为 1..25,000，完整 MCP response 仍受 1 MiB 硬门约束；
- 最大 16×16、level 2 投影为 22,802 个声明内 lineage element，序列化结果低于 1 MiB；超预算、未知字段、root 篡改、重复 edge 或错误 quad root 均 fail closed；
- `complete=true` 只表示 `all-root-mappings-within-declared-preview-lineage` 无截断，不代表全 V/E/F/C、逐级 child path、corner domain、influence weight、artifact/readback sidecar 或 GLB identity；
- 调用不写 SQLite/CAS/candidate/version，`artifact_binding_status=unavailable-preview-only`、`materialization_status=preview-only-not-persisted-in-glb`、`quality_status=structural_only`。

该 historical slice 当时源面是 168 schemas、20/20 active operators、46 read + 33 opt-in write = 79 tools。该 slice 仅为 `PASS_SOURCE_STRUCTURAL_ONLY`；package/live/render/visual/human/PBR/export-restart/HQ360 不变。receipt：`docs/evidence/mcp010f/blender-subdivision-topology-lineage-source-gate-20260819.json`。

`SubdivisionCreaseEvaluationRequest@1 → SubdivisionCreaseEvaluationResult@1` 与 `forgecad.geometry.subd-cage@2` 是第一个真正执行 edge crease 的 product-owned slice：

- 仅接受 3..16 × 3..16 regular open quad grid、1..2 subdivision levels、1..128 条 lexicographically canonical interior adjacent edge；sharpness 只允许整数 1 或 2，并在每级后减 1；
- fixed `edge-only` boundary 把 boundary edge 视为 sharp；普通边界顶点按 two-neighbor crease mask 求值，三条及以上 sharp edge 的 junction 才 corner-pin；显式 boundary crease 因冗余被拒绝；
- Worker 的 smooth/dart/two-edge crease/three-edge corner/boundary junction/level decay golden、重复 GLB byte/hash、strict readback 与 negative matrix PASS；
- Runtime 的只读 branch 先归一化 crease set 再绑定 input hash，独立 result validator 回绑原 request；它不编译 mesh或写状态；
- actual program 已通过既有 `geometry_prepare` 产生 128-triangle candidate、ArtifactReadback@2 hard gate 和零 version，证明 operator 不只是 schema/projection；
- `hard-surface-detail@0.2.0`、OperatorCatalog、Agentic action/critic/repair allowlist 已同步；Modifier Stack v1 明确保持不接纳 @2；
- Blender frozen GPL source 和 OpenSubdiv fixed source只用于语义研究，未复制、链接、安装、执行或成为依赖。

该 historical slice 当时源面是 166 schemas、20/20 active operators、45 read + 33 opt-in write = 78 tools。该 slice 仅为 `PASS_SOURCE_STRUCTURAL_ONLY`；package/live/render/visual/human/PBR/export-restart/HQ360 不变。receipt：`docs/evidence/mcp010f/blender-subdivision-crease-source-gate-20260819.json`。

`ParametricDesignKitRequest@2 → ParametricDesignKitProgram@2` 以 clean-room 方式学习 Blender Node Group 的可复用接口/实例分离，但只提供三个产品编译内置 template：

- `forgecad.group.rounded-box@1`、`forgecad.group.mirrored-box@1`、`forgecad.group.arrayed-cylinder@1`；
- group definition 不可变，实例只能填充 closed typed single-value sockets，同一 template 跨实例共享 template hash，parameter/instance/program hash 独立；
- Runtime 生成确定 evaluation order 与 source map，并将 3-node DAG 交给现有 Geometry Worker 校验；
- 不允许 nested group、field、attribute、simulation、script、path、URL、environment、network 或 dynamic plugin；
- 复用现有只读 `geometry_program_hash`，不新增 tool/operator/Skill/dependency，不写 candidate/CAS/version。

source receipt：`docs/evidence/mcp010f/blender-parametric-group-v2-source-gate-20260818.json`。该切片是 `structural_only`，不能表述为 Geometry Nodes、Blender addon 或 Python 生态已配置。

`GeometryModifierStackRequest@1 → GeometryModifierStackProgram@1` 复用现有只读 `geometry_program_hash`：

- 输入绑定 project、representation plan、Part、MaterialZone、solid、一个 source base node、1..8 个有序 modifier 和 `input_sha256`；
- modifier ID 必须唯一；字段集合 closed；未知字段、错误 hash、非法 operator、非法参数由 Runtime/Geometry Worker fail closed；
- 当前只允许已经 active 且真实执行的一元 `transform@2`、`mirror@1`、`array@1`、direct-box `bevel@1` 与 corner-domain `normal-policy@1`；
- disabled modifier 保留在 authoring/evaluation trace 中，但不生成 DAG node，也不改变 effective evaluation hash；
- enabled modifier 按顺序 lowering 到严格 `GeometryProgram@2` 输入链；重排会改变 stack/program hash；
- 返回每阶段 input/output evaluation hash、effective node、完整 canonical GeometryProgram 和明确 limitations；
- 调用不编译 mesh，不创建 candidate/Job/version，不写 SQLite/CAS，不绕过 `geometry_prepare → readback → quality → user approval → confirm`。

该 slice 是 `structural_only`，不是 Blender Modifier 系统完成，也不是视觉质量 PASS。

`TopologySnapshotRequest@1 → TopologySnapshot@1` 由新增只读 `topology_snapshot_get` 消费：

- 必须先取得 candidate-bound、hard-gate-passed 的 `ArtifactReadback@2`，并逐项绑定 project/candidate/artifact/readback/program/catalog/readback-config/policy hash；
- 单次只读取一个 Part，最多完整投影 512 faces、1536 vertices/edges/corners，canonical response 最多 1 MiB；任何上限超出都 fail closed，不返回前缀或截断结果；
- 输出确定性排序的 vertex/edge/face/corner 表、邻接、boundary/non-manifold/orientation conflict、face winding、Part/source-node/MaterialZone 与 corner normal/UV/tangent；
- `topology_space=evaluated-glb-triangle-mesh@1`、`id_scope=artifact-bound`、`cross_version_stable=false`、`source_lineage_status=partial-operator-node-only`，不把 GLB 三角面冒充 BMesh/edit cage；
- primitive、开放 `subd-cage@1`、bounded same-Part Boolean、重复 hash、错误绑定、预算与 MCP unknown-field focused tests PASS；调用不新增 candidate/CAS/SQLite/version。

该 slice 同样只提供 `structural_only` readback；Boolean operand lineage 与 SubD control-root→最终 evaluated quad topology preview 已由后续独立工具补齐，但持久 authoring element ID、逐级 child path、corner/weight lineage 和 artifact-bound sidecar 仍未实现。

`SubdivisionEvaluationRequest@2 → SubdivisionEvaluationResult@2` 复用同一个只读 `geometry_program_hash` 与现有 `subd-cage@1` Worker：

- 输入仅允许 2..16 × 2..16 的规则矩形开放控制网格、0..2 级 uniform Catmull–Clark-style refinement，`solid=false`；
- policy 固定 `edge-and-corner` boundary、crease unsupported、adaptive=false、limit-surface=false；未知策略或额外字段 fail closed；
- 分配前以 checked/bounded 公式投影 control/evaluated vertex、edge、quad、triangle 与 boundary edge 数，并要求 `max_triangles` 足够；
- 输出绑定 input/control-cage/policy/topology/program/catalog/canonical hash，并返回可由既有 Worker 编译的完整 `GeometryProgram@2`；
- normals 明示为 Worker 重新生成的 smooth normals，UV 是三角 chart 后处理，tangent 是 pinned MikkTSpace 0.3.0 后处理；不冒充 Blender/OpenSubdiv face-varying interpolation；
- 调用不创建 candidate/version/Job/CAS/SQLite 状态，仍须后续走正常 prepare/readback/quality/approval 链。

该 slice 是 ForgeCAD 自有有界 source/structural 能力；Blender GPL 与 OpenSubdiv TOST-1.0 仅保留研究状态，未链接、执行或复制进 Runtime。

`RenderProfile@1` 与 `RenderSet@2` AOV lineage 已由 Render Worker/Runtime 共享并 fail closed：

- 固定 `forgecad-fixed-software@2` / `cpu-raster@1` / `forgecad-renderer-2`、512×512、2× axis supersample、无 RNG/adaptive/temporal/motion blur；
- beauty 是唯一 fixed linear-to-sRGB display-color pass；silhouette/depth/normal/AO/part-ID/material-ID/wireframe/UV-stretch 都是 non-color data；
- depth 明示为 reversed normalized RGBA8、不是 metric distance；normal 为 signed unit vector → unorm8；ID pass 绑定固定 palette definition hash；
- Worker `render_glb` response 返回完整 profile，Runtime 与两个 `RenderSet@2` producer 精确绑定 canonical/AOV/color-pipeline/ID-palette hashes；Schema 同步固定 exact fields、AOV order 与 beauty/data color space；tamper、缺字段或不同 profile fail closed；
- Runtime adapter 在 RenderSet 生产前实际解码每个 PNG，要求声明尺寸与 RGBA8 完全一致；Part-ID/Material-ID palette 只允许 mesh/material index 0..255，溢出由 Render Core 拒绝；
- 无新 MCP 工具或持久写入，不引入 Blender、Cycles、EEVEE、OCIO、GPU、EXR、透明 film、custom AOV 或 compositor。

receipt：`docs/evidence/mcp010f/blender-render-profile-aov-lineage-source-gate-20260818.json`。该 slice 只证明 source/structural contract，不证明 package/live 或视觉质量。

`MechanicalRestFrame@1`、`MechanicalPoseAction@1` 与 `MechanicalPoseEvaluationRequest/Result@1` 提供独立的机械层级姿态投影：

- 请求精确绑定 project、candidate、artifact、`ArtifactReadback@2`、GeometryProgram、OperatorCatalog 与 readback config hashes，并从同一 GLB 的 Part/source-node lineage 验证每个 link；
- rest frame 最多 64 links，要求唯一 root、完整且无环的 parent map、最大深度 16；每个 link 只能是 fixed、单轴 revolute 或单轴 prismatic；
- action 固定 1000 Hz 整数 tick、linear interpolation、clamp extrapolation、unkeyed=`rest`，最多 64 channels、每通道 32 keys、总 keys 512；
- Runtime 对输入排序归一化、binary64 量化与 quaternion 符号 canonicalization 后计算 local/world TRS；同义重排返回相同 rest/action/evaluated pose hash；
- 默认只读 MCP `mechanical_pose_evaluate` 使用 closed inline transport Schema，未知 `script`/`bpy` 字段在进入 Runtime 前拒绝；Runtime 继续执行完整跨字段与 lineage 校验；
- 调用前后不创建 candidate/version/Job/CAS/SQLite 状态，不调用 Worker，也不 materialize 新几何。

receipt：`docs/evidence/mcp010f/blender-mechanical-pose-source-gate-20260818.json`。该 slice 不是 Armature、骨骼蒙皮或可播放动画资产，也没有 package/live/视觉/真人证据。

`MechanicalAnimationClip@1` 在上述只读姿态投影之上提供一个 Runtime-owned durable 原子能力：

- 显式 opt-in `mechanical_animation_clip_prepare` 只接受 closed、candidate-bound、非空 PoseAction 与最多 16 个严格递增整数 tick；
- 写入前从 durable GeometryCandidateEvidence/CAS 重读 source GeometryProgram 与 GLB，由实际 fixed Geometry Worker 连续重编译两次，要求与 candidate artifact 全 GLB bytes 完全一致且两个 Worker cohort 非空、相同；
- canonical clip 写入 CAS，SQLite Link 精确绑定 project/candidate/artifact/readback/evidence/program/catalog/config/request/rest/action/clip/cohort hashes；重复请求幂等，冲突 fail closed，提交失败只回滚本次新建且尚未链接的 temporary clip；
- 默认只读 `mechanical_animation_clip_get` 可跨 Runtime 重启验证 CAS canonical bytes、SQLite Link 与 live durable evidence；`mechanical_animation_clip_preview_get` 只允许 immutable schedule 内一个 tick，并对 transient derived frame 进行两次 Geometry Worker exact replay，不写 candidate/version/CAS；
- 新工具保持 Runtime 唯一写者与 MCP 薄适配器边界，不接受 script、Python、path、URL、environment、network 或 dynamic plugin。

receipt：`docs/evidence/mcp010f/blender-mechanical-animation-clip-source-gate-20260819.json`。该 slice 仍是 caller-authored rigid Part action，不是 Armature/bone/skin、IK/constraint、NLA/F-Curve/driver/timeline、GLB animation channel、Blender add-on 或 Python 生态，也不证明视觉质量。

2026-08-20 的 Viewer adaptation 只复制 Blender Dope Sheet 的“时间轴与通道层级分开、界面状态不写回数据真值”这一交互原则，不复制 Blender 源码、图标或 GPL 实现。ForgeCAD 的 bounded clip inventory、verified clip detail 与 scheduled single-tick frame 都通过认证只读 IPC；React 只选择 immutable clip/tick，并把 Runtime 双 Worker 已验证的 rigid Part delta 应用到 exact-one identity GLB Part owner。Viewer 不本地求值姿态、不自动连续播放、不调用 prepare/confirm；embedded animation、Bone/SkinnedMesh 和 owner 歧义 fail closed。这不是编辑器、Armature/skin/IK/NLA/F-Curve/driver、GLB animation 或 visual PASS。

## 4. 后续原子路线

严格保持一次一个原子任务和既有任务顺序：

1. 已完成：bounded `TopologySnapshot@1` evaluated artifact topology projection；authoring/source topology 仍是后续独立任务。
2. 已完成：bounded `bevel@1 + normal-policy@1` source slice；只允许 direct box 的全部 12 edge 与 corner-domain face-area×face-angle normal，并精确回读 source-box lineage；通用 topology 仍延期。
3. 已完成：Modifier evaluation v2 source slice；显式 input/evaluation/output hash、dirty reason、cache key、catalog cohort 与 disabled trace-only reuse；apply 仍走 Runtime prepare/confirm 事务，持久 mesh cache/build cohort 未实现。
4. 已完成：bounded Subdivision evaluation v2 contract/policy；规则开放 quad grid、boundary/crease/UV/normal/tangent/budget 与缺失 lineage 已显式化。通用 SubD/OpenSubdiv/limit surface 仍延期。
5. 已完成：fixed RenderProfile/AOV color-data lineage；固定 engine/profile、九 AOV、scene-linear/display transform metadata、renderer profile/cohort hashes。camera/material 更完整的独立 lineage、package/live/visual 仍延期；EEVEE/Cycles 仅作接口分层研究。
6. 已完成 source + Viewer read-only slice：Mechanical pose projection 与 Runtime-owned immutable `MechanicalAnimationClip@1`；source GLB 同 cohort 双重 exact replay、CAS/SQLite durable Link、重启读回、scheduled single-tick transient 双 Worker preview，以及 Viewer 的 clip inventory/detail、临时离散 tick/通道/rigid-link evidence inspector。skin、IK、NLA/F-Curve/driver、角色动画编辑器、3D playback、GLB animation channel 与真实可编辑时间轴仍延期。
7. 生态：UI、MCP、未来 SDK 共享 closed Core Command；第三方能力只能以签名、版本固定、typed IPC、离线、可移除的隔离 Worker 或 first-party Skill 进入。
8. 只有到 MCP012/013 的供应链、打包、恶意输入、许可证、退出方案和真人门完成后，才评估可选外部 Blender renderer adapter；它永远不能成为 Runtime 状态写者或静默替代固定 renderer。

## 5. 验收与禁止宣称

每个 slice 必须分别记录：实现、自动测试、真实 Runtime/packaged transport、视觉/真人 Gate。以下当前事实不因本轮 source Gate 改变：

- `QUALITY_TARGET_NOT_MET`
- `INCOMPLETE_TRUTH_BINDING`
- camera `MISMATCH`
- benchmark `BLOCKED_INCOMPLETE_BINDING`
- `HQ_360 = BLOCKED_REFERENCE_COVERAGE`
- Cycles/EEVEE、通用 BMesh、通用 Subdivision、通用 Bevel/Weighted Normal、Armature/skin/完整 Animation 与 Python plugin ecosystem 均未配置完成。

## 6. 固定研究入口

- Blender commit：`https://github.com/blender/blender/commit/72ccdd6e96ca119a1ffa3372559cc5654343b477`
- License：`https://github.com/blender/blender/blob/72ccdd6e96ca119a1ffa3372559cc5654343b477/COPYING`
- BMesh：`source/blender/bmesh/`
- Modifier：`source/blender/blenkernel/BKE_modifier.hh`、`source/blender/modifiers/`
- Depsgraph：`source/blender/depsgraph/`
- Boolean/Bevel/Subdivision/Weighted Normal：对应 `source/blender/modifiers/intern/` 与 `source/blender/geometry/`
- Render/EEVEE：`source/blender/render/`、`source/blender/draw/engines/eevee/`
- Cycles：`intern/cycles/`
- Animation/Rig/RNA：`source/blender/animrig/`、`source/blender/makesrna/`、`source/blender/python/`

这些入口只用于可追溯研究，不能直接复制到产品树。
