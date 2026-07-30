# ForgeCAD 当前原子任务索引

版本：2026-07-29
状态：精简后的唯一任务状态表

## 1. 使用规则

- 一次领取一个任务；同一时刻最多一个 `in_progress`；
- `ready` 的全部依赖必须为 `done`；
- `blocked/external` 必须写清缺失证据；
- `superseded` 只表示由新任务接续，不表示原目标通过；
- 完成必须同时更新代码/合同/测试/文档；聊天摘要不能改变状态。

## 2. 当前任务表

| Task ID | Status | Dependencies | Outcome / boundary |
| --- | --- | --- | --- |
| FGC-S001 | done | - | `ActiveDesignSnapshot@1` 单一状态基线 |
| FGC-D003 | done | S001 | 旧四领域含糊输入澄清 Alpha 基线 |
| FGC-T001 | done | S001, D003 | 工作台状态机和 Agent-first E2E 基线 |
| FGC-B001 | done | S001 | 备份/恢复和导入对象边界 |
| FGC-K003 | done | S001, B001 | Rust-owned 产品状态；Python 只执行受限几何 |
| FGC-G826 | done | K003 | Surface provenance、UV/tangent、严格 GLB readback |
| FGC-F026 | done | T001, K003 | Codex 式工作台、一个结果、单 renderer |
| FGC-V003 | done | F026, K003 | 一次合成、有界修复、唯一未保存结果 |
| FGC-A005 | done | K003 | 受限表面程序、preview→confirm、PBR lineage |
| FGC-R007B | done | A005, V003 | 参考→新 GLB exact-lineage 工程闭环；不证明相似度 |
| FGC-M108A | done | K003 | 双档 GLB/PBR/readback/quality/export 工件 |
| FGC-VP201 | done | K003, G826 | `ForgeVisualProgram@2` 最小 typed DAG/lowering |
| FGC-VP202 | done | VP201 | 纯宏、有界 repeat 与静态预算 |
| FGC-VP203 | done | VP202 | 高层 profile/loft/sweep/boolean/array/mirror |
| FGC-VP204 | done | VP203 | 一次 author + 最多一次 patch、缓存和恢复 |
| FGC-U001 | done | VP204 | ADR-0022、产品范围与任务迁移 |
| FGC-U001A | done | U001 | 删除旧文档并把有效内容并入新权威文档 |
| FGC-U002 | done | U001A | `SubjectProfile/VisualFeatureContract/RepresentationPlan`、统一 multimodal request、删除模板回退 |
| FGC-U003 | done | U002 | `UniversalAssetSource`、component/detail/material/projection 与统一 lineage |
| FGC-U004A | done | U003 | DeepSeek/千问唯一 AI Provider；删除 Fal/远程 Mesh 运行时入口并建立 fail-closed Gate |
| FGC-U004 | in_progress | U004A | procedural/deformable/local-hybrid 能力、Appearance Compiler 与 readback |
| FGC-U005 | blocked | U004 | 八类真实输入、1+1 时间成本和独立真人盲评 |
| FGC-E005 | superseded | VP204 | 只保留 mechanical procedural regression，不阻塞 U002 |
| FGC-C111B | superseded | V003, A005 | 工程/时间/显示回归保留；reference/human 未通过 |
| FGC-P009 | external | K003 | 原生签名、安装、升级与发布环境 |
| FGC-L001 | blocked | P009 | 删除 legacy 写路径并保留最小只读转换 |

Next unblocked task IDs:

```text
FGC-U004 (in_progress)
```

## 3. FGC-U001A 文档清理任务卡

状态：`done`

范围：删除完全被取代的 ADR、R0–R6/旧 Weapon evidence 和独立 legacy 手册；将仍有效的兼容、质量和执行边界并入当前权威文档；不修改运行时能力。

退出条件：

1. 当前树不再包含旧产品操作手册和无当前入口的阶段报告；
2. 所有 Markdown 相对链接有效，当前/目标/兼容状态一致；
3. 用户指南不产生通用能力 overclaim；
4. docs walkthrough、repository integrity、safety scope、secrets、agent check 和 `git diff --check` 通过。

完成证据（2026-07-29）：Markdown 从 105 份收敛到 54 份；删除 4 份独立 legacy 手册、12 份被取代 ADR 和 35 份无当前入口的历史 evidence；有效兼容规则已并入 `COMPATIBILITY_MIGRATION.md`；六项退出 Gate 全部通过。

## 4. FGC-U002 通用入口任务卡

状态：`done`（2026-07-29）；在当前脏工作区原位合并并保留 E005、VP204、视觉 Provider 与工作台拆分的并行改动。

范围：建立类别开放的视觉理解和表示规划合同，接入真实产品 author request，删除未知对象到 C111/机械臂/未来武器的静默回退；不在本任务实现 deformable/local-hybrid、角色骨骼或全部类别质量。

退出条件：

1. `SubjectProfile@1` 覆盖身份、部件、macro/meso/micro、轮廓、姿态、材质、视图、遮挡和不确定性；
2. `VisualFeatureContract@1` 把高显著性特征绑定到 sealed evidence 和验收视图；
3. `RepresentationPlan@1` 逐部件选择声明 capability；类别未知不等于 unsupported；
4. Domain Pack 只提供知识提示，所有模板回退在 Provider/worker 前拒绝；
5. 纯文本、真实单图、多视图和已有资产进入同一 sealed author request；
6. 没有可执行表示时返回 typed limitation，零资产/Snapshot/导出副作用；
7. schema、生成类型、Rust tests、产品 E2E、contracts/types 和基线 Gate 通过。

完成证据（2026-07-29）：新增 `UniversalAuthorRequest/Outcome@1`、`SubjectProfile@1`、`VisualFeatureContract@1`、`RepresentationPlan@1`、`RepresentationLimitation@1` 和 `VisualEvidenceGraph@2`；Rust capability registry 仅开放 `procedural.robotic_arm_visual_v1`；新项目只广告 `author_universal_asset`；猫、树、建筑、人物与其他未具备表示的对象不再生成 C111/机械臂模板。U002 合同 Gate、协议/核心/app-server focused tests、G1–G7、类型/生产构建、U002 工作台与 R3 Snapshot/导出/重启回归全部通过；未调用收费 Provider。

## 5. FGC-U003 通用资产源任务卡

状态：`done`（2026-07-29）；在当前脏工作区原位合并，未调用收费 Provider。

范围：建立 Rust-owned `UniversalAssetSource@1` 及 component/detail/material/camera/projection 合同，把当前可执行程序化结果追到输入合同、设计 revision、ShapeProgram、GLB/PBR readback 和固定验收视图；不在本任务开放 deformable/local-hybrid 或伪造照片 PBR 恢复。

退出条件：

1. Provider 不能直接提交或推进 `UniversalAssetSource`，Rust 从通过 U002 校验的 request/profile/feature/plan 和当前程序化 revision 派生；
2. 每个 Subject Part、Visual Feature 和 Material Zone 都有稳定 source binding，重复、悬空、跨合同 hash 漂移和未知表示 fail closed；
3. 无拟合证据的相机只能标为 unresolved；projection layer 必须绑定已验证 camera、派生 evidence 和未观测 texel mask；
4. 编译后的 source 精确绑定 ShapeProgram、GLB、语义/编译 readback、artifact profile、renderer 与全部固定视图 hash；
5. 完整 source 留在 Rust candidate/preview 与确认版本 provenance，工具只返回 hash/count 摘要，不形成第二数据库版本头；
6. U002 limitation 和现有资产保留语义不变，未支持类别仍为零几何副作用；
7. Schema/生成类型、focused Rust/app-server、U002/工作台回归、G1–G7 和基线 Gate 通过。

完成证据（2026-07-29）：新增五份 JSON Schema、Rust validator/builder、代码所有 source hash 和编译绑定；当前机械臂 author→build→preview 可验证完整谱系，确认时将 source 与 semantic hash 写入同一 AssemblyGraph provenance。`agent:u003-universal-asset-source-gate`、U002 回归、Rust/桌面构建、工作台与文档 Gate 同轮通过。真实相机求解、UV rasterization、照片 PBR 恢复、新类别 GLB 和真人质量均保持 `NOT RUN`。

## 6. FGC-U004 本地混合表示执行任务卡

状态：`in_progress`（2026-07-29）；Provider 主权前置切片 `FGC-U004A` 已完成，未调用真实收费 Provider。

范围：由千问读取 sealed 参考形成视觉证据，DeepSeek 编写受限设计源，将 procedural/deformable/local-hybrid 按已声明 capability 接入同一 `UniversalAssetSource`、严格 readback、Part/Zone、版本与导出真值；不依赖第三方远程 Mesh API，不因类别入口开放而提前开放 capability。

U004A 完成证据：删除 Fal/Hunyuan 远程生成 adapter、Tauri command、凭据、恢复 probe、TypeScript transport 和工作台入口；DeepSeek 配置只接受官方 `api.deepseek.com + deepseek-*`，千问视觉配置只接受官方 `aliyuncs.com + qwen*`；`release:ai-provider-policy`、反向 Rust tests、desktop typecheck/build 和 F026 单视口 smoke 通过。旧 remote-job schema/migration 只作兼容读取，`mesh_seed.generic_v1` 保持 unavailable。

2026-07-30 P0 质量证据切片：前端 `WorkbenchPbrVisualCapture@1` 将已解析的编译 GLB 精确 SHA-256 绑定到唯一 Three.js renderer；采集只接受 `forgecad-workbench-pbr@1`、同一 GLB、sRGB/ACES、固定 render-manifest hash 与 generic `turntable_000…315` 八视图，拒绝 external reference、参数回退和 Python software raster。每一帧都会临时使用同一个 renderer 的固定 `640×640` drawing buffer；Rust-issued session 固定该尺寸并从上传 PNG 的 IHDR 重读宽高，实际相机 world/projection/target/取景矩阵共同形成 pose hash，避免窗口尺寸或槽位名伪装成可复现视觉证据。Core `CandidatePbrCaptureSession@1` / `CandidatePbrCaptureEvidence@1` 绑定候选 GLB/readback/renderer/nonce/短时预算；Rust app-server 已实现一次性短期 registry：OS 随机 nonce、精确 Project/Turn/execution/readback 重检、PNG hash/尺寸/预算/视图检查、LRU/TTL、取消和消费即清除，重放或非法提交无候选、preview、Snapshot、Quality、Export、Version 副作用。Tauri 受限 `issue/submit` command 已经把瞬态 GLB 和有界 PNG 上传接到该 registry，nonce 不出 Rust，提交后证据必须被原子采用；同一候选未持有 capture 时，类别开放 route 的评估会以 `CANDIDATE_PBR_CAPTURE_REQUIRED` 停止，不能回退 Python software raster。带 sealed 图片的 universal route 从 `VisualEvidenceGraph@2` 生成 Rust-only 比较声明投影；capture 后只有用户点击工作台授权卡，Tauri 才为该精确 Project/Turn/GLB/request/graph/policy scope 创建短时千问预算 grant 并回写执行态，点击前没有网络调用。续跑前桥接层重算 scope；hash 或候选漂移、无 grant、已失效 grant 全部阻止 evaluate/preview。patch 后的新 GLB 清除旧 grant，必须重新 capture 和授权。Turn 进入持久化 `waiting_for_capture`，授权并 capture 被采用后 ActionLoop 保留封存 request，仅重跑 evaluate→preview；失败时仅调用一次 DeepSeek typed patch，第二次失败终止且无永久副作用。重启不会猜测恢复内存 continuation。它们均不落盘，C111B QA 独立保留。真实千问输入、packaged 八视图 E2E 与真实未见输入仍未完成，因此不得写成“已完成真实 PBR 质量验收”。

2026-07-30 P0 GPU auxiliary pass slice：每个已绑定的 beauty 视图现在还必须由同一 `forgecad-workbench-pbr@1` context 生成一张 `960×640` contact sheet，按固定 tile 顺序包含 silhouette、normal、depth、part-ID、material-ID 五个 `320×320` pass。前端以 offscreen `WebGLRenderTarget` 和同一 camera/scene/renderer 读取 auxiliary pixels，不创建第二 WebGL context；part/material pass 用稳定 hash color 编码，临时材质、overlay 与可见性在 capture 后恢复。Tauri 传输每视图 beauty+auxiliary PNG；Rust 验证两张 PNG 的 IHDR、每图 SHA、固定 auxiliary 尺寸、独立字节上限及总预算，缺失/错误尺寸即不采用 capture；检查拒绝空白或全图单色，但允许合法二值 silhouette/单 zone ID pass。`desktop:u004-workbench-pbr-capture-playwright` 已在真实 browser canvas 产生完整五通道 bundle；这仍只用 Rust fixture，不能替代 packaged 设备证据。Qwen 比较仍仅接收同源 PBR beauty；辅助 pass 为 Rust-bound deterministic evidence，不可被作为视觉相似度通过的替代。尚未完成 packaged 设备真 GPU E2E、auxiliary pass 的语义指标、真实千问比较或未见输入评测。

U004 P1 排序：不新增第二 DSL。VP203/VP204 `ForgeVisualGeometryProgram@2`（profile/extrude/revolve/loft/sweep/boolean/mirror/array/Part/Zone，source-map/readback/单 patch）现已接到 `UniversalAssetSource@2` 与唯一 ActionLoop：`procedural.generic_hard_surface_v1` 只能通过 source hash、同源 PBR capture、VP204 stable node/material patch 与重新 capture 的候选链执行，绝不回退 C111/机械臂。2026-07-30 已补齐同一 DSL 对 restricted worker 已有 `cylinder/capsule/wedge/radial_array/bevel_approx/surface_panel` 的 typed 表达和 fail-closed 参数限制；同日新增 `GenericHardSurfaceAppearanceCompilation@2`，由 Rust 从 sealed profile/feature/material-zone 选择一至八个真实外观区，逐区生成并校验 normal-relief/roughness/条件式 emissive `SurfaceLayerProgram@1`，并将 plural sealed lowerings 接入现有 production 五通道 PBR GLB worker。照片外观投影仍未烘焙，但其合同现要求 evidence artifact、unobserved texel mask 与已拟合相机属于同一 sealed view，且 artifact 的通道必须匹配 BaseColor/Normal/Roughness/Metallic；unresolved 相机、跨图拼接或将猜测标为 observed 全部拒绝。它不是第二 DSL、不是 Provider 自报材质、不是 UV/多区照片纹理恢复；patch 会重新派生该层。只有该 source-kind 完成真实千问比较、packaged E2E、preview→confirm/version/Snapshot/export 与未见输入评测，才可标为已实现能力；角色/生物/植物等仍等待各自表示，不得借 generic hard-surface 或 C111 模板执行。

2026-07-30 P4 游戏交付真实性：`GameAssetProfile@1` 只能在 UAS@2 planned state 封存 LOD、collision Part、socket 和目标 texel density；它们不是由 1K PBR 或单 mesh 自动推导出的交付事实。`compile_game_asset_lod_delivery` 追加真实 LOD1/2 index accessors 与 `MSFT_lod`，并以 sealed source/profile 重算拓扑、预算和 2% 全资产尺度误差。`derive_game_asset_delivery_bindings` 只从已验证 UAS@2 将 SubjectPart 映射到 GLB `forgecad_feature_node_id`；`compile_game_asset_delivery` 再以该 mapping 写入 off-scene AABB collision meshes 和 socket pivot/forward quaternion nodes，`verify_game_asset_delivery_glb` 重算其 bounds、index、transform 和 lineage。最终 LOD0 的 `GameAssetTexelDensityReadback` 从真实 POSITION/UV0 三角面和嵌入 base-color PNG 的 IHDR 尺寸计算有效 pixels/m，逐 material zone 封存实测值；profile target 只形成 `target_met`，绝不替代测量，`target_met=false` 不可附着 delivery receipt。runtime 先保留 source LOD0 给 PBR/比较，再以 Rust 派生 delivery GLB；确认现在使用专用双工件原子事务保留 source LOD0、delivery GLB 和 interactive preview。`:model.glb`、`:export` 与 `ForgeAssetPackage@2` 都在重新验证同一 source/profile/binding/receipt 后返回 delivery bytes，并将 source hash 与 delivery hash 同时封存。非游戏 UAS@2 preview 继续使用独立 `NativeUniversalPreviewProvenance@1`，不重用旧 V003/Domain Pack 谱系。C111 smoke 已通过完整 readback；不达质量界限的预算继续返回 `GAME_ASSET_LOD_QUALITY_BUDGET_CONFLICT`。尚缺真实 valid source→delivery 的 preview→confirm→export/package E2E、幂等/篡改、用户 profile 选择、packaged 设备和用户级交付证据，后续 P4 必须补齐这些产品交付要件。

2026-07-30 P2 受限像素烘焙基础：sidecar worker 保留 `ReferenceUvEvidenceBake@1` 的 declared-rectangle 兼容覆盖，并新增真三角面路径 `ReferenceCameraUvRasterBake@2`。V2 只接收 SHA-256 封存 PNG、明确 retained Design Surface zone、受限的最终 GLB positions/UV0/indices 与 row-major world-to-clip；Worker 用所有 zone 深度 raster，只有可见 target zone 的 source pixels 才落入真实 UV texel，其余程序化 base-color 不变并由 inverse `unobserved_texel_mask` 标记。GLB receipt/readback 现在严格区分 V1 与 `ReferenceCameraUvRasterBakeReceipt@2`，后者绑定 source/camera provenance、matrix hash、triangle count、map/mask hash 与 coverage。`RestrictedGeometryExecutionRequest@1` 继续拒绝无 retained layer、跨/重复 zone 或 render 请求携带此输入；focused direct + compile→同一 GLB/readback 回归共 36 passed。P2.3 已由 Rust 从 exact candidate GLB SHA、最终 compile-readback bounds 和冻结八视图导出 `ProjectionCameraBinding@1`；capture session 固定每视图 binding SHA，app-server 重验 Project/Turn/session 后才返回绑定，工作台以唯一 Three.js renderer 应用它并在上传证据中回传同一 SHA。P2.4 已将 sealed source 闭合在 Rust：`CoreRepository` 只按同 Project evidence ID 读取 immutable `reference` CAS relation，V2 builder 重验 `ReferenceEvidence`、`image/png`、CAS SHA、8 MiB/worker PNG profile、zone ID 与可复验 binding，再生成不持久化的 DTO；跨 Project、byte/hash、zone 或 camera drift 全部拒绝。该链将相机和参考图从 Worker 的外部前提变为候选 GLB/Project 谱系的一部分。尚没有多视图融合、工作台图片投影入口或完整 ActionLoop wiring，故不得视为用户可用照片投影能力。

2026-07-30 P3 受限本地形变与逐部件 Hybrid 执行切片：runtime manifest、JSON Schema、Python semantic validator、Rust G819 validator 和 `ForgeVisualGeometryProgram@2` 同时新增 `lattice_deform`。它不是任意网格编辑：仅接收一个较早 mesh、固定八个 `corner_offsets` 组成 `2×2×2` trilinear cage，偏移逐轴封顶为源 AABB 的 `±25%`，且必须非零；profile、旋转穿透、越界 cage、degenerate bounds/triangle 全部 fail-closed。Worker 不增加三角数、不导入外部 mesh、不执行代码，仍保留原 Part/Zone/material lineage 和 feature readback。`deformable.local_lattice_shell_v1` 已由 Rust capability registry 选择；UAS@2 现可在同一资产中让部分 Part 保持 `procedural.generic_hard_surface_v1`，让明确 Part 使用 lattice，并重验 terminal、offset、计划映射和 component binding。author/build/readback/preview lineage/同源 PBR capture/visual convergence/一次 VP204 patch 均走同一 ActionLoop 路由。通过 Python deterministic/readback 4 tests、Core UAS focused、app-server local-hybrid author/build focused、ActionLoop route focused 与 `contracts:types:check`。这仍只开放可审计的硬表面局部混合；不等同任意 mesh、角色、生物、mesh seed 或神经重建，真实千问、packaged E2E、确认版本/导出和跨类别质量仍未运行。

剩余退出条件：

1. 为 local-hybrid 补齐真实工作台与 packaged E2E、完整 confirm/version/export 与真实千问比较证据；非游戏 UAS@2 的确认 provenance 已接入现有单 GLB 事务，但必须用真实 GLB 端到端覆盖；游戏 delivery 另需 source LOD0 + delivery GLB 的双工件原子持久化；其 source、参数、预算和 readback Schema 已冻结并由 Rust capability registry 选择，DeepSeek/千问/前端不能自报 executable；
2. 每个表示绑定 request/profile/feature/plan/source、CAS、Part/Zone、GLB/PBR、固定视图和未观察区域；
3. 建立角色/生物/植物/布料等所需的受限曲面、骨架/形变与局部细节算子；任意代码、动态文件/URL 和无界生成继续拒绝；
4. 千问固定多视图比较与 Rust hard gate 通过后，候选才进入唯一 `SingleResultDecision`、preview→confirm、不可变版本、Snapshot 和导出；
5. Provider 取消/超时/非法输出与本地编译失败保持零永久副作用；不存在的表示继续 typed limitation；
6. 八类真实未见输入证明身份、轮廓、结构和材质净提升，失败不回退模板；完整 U004、U002/U003、工作台、G1–G7、文档和发布基线 Gate 通过。

## 7. 冻结回归边界

- C111B/E005 fixture、预算、readback 和时间证据继续回归，但不能写成通用质量；
- M108A 只证明 production 工件，真人视觉质量归 U005；
- Weapon/Concept 兼容任务只按 `COMPATIBILITY_MIGRATION.md` 推进；
- 未经用户精确授权，不运行真实 DeepSeek/千问付费调用、30 题批次或真人评审；第三方 AI Provider 永久禁止。
