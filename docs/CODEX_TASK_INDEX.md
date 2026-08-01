# ForgeCAD 当前原子任务索引

版本：2026-08-01
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
| FGC-U004 | in_progress | U004A | procedural/deformable/local-hybrid 能力、Appearance Compiler、真实 GPU/PBR capture provenance 与 readback |
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

完成证据（2026-07-29）：新增 `UniversalAuthorRequest/Outcome@1`、`SubjectProfile@1`、`VisualFeatureContract@1`、`RepresentationPlan@1`、`RepresentationLimitation@1` 和 `VisualEvidenceGraph@2`；Rust capability registry 仅开放已审查程序化 capability；新项目只广告 `author_universal_asset`；猫、树、建筑、人物与其他未具备表示的对象不再生成 C111/机械臂模板。U002 合同 Gate、协议/核心/app-server focused tests、G1–G7、类型/生产构建、U002 工作台与 R3 Snapshot/导出/重启回归全部通过；未调用收费 Provider。2026-07-31 的本机开发作者切片新增 `procedural.generic_visual_exterior_v1`，以 Rust-sealed 的类别外观代理让这些对象可以产生不同的轻量 GLB；它不改变 U002 的“不得模板回退”边界，也不宣称正式跨类别质量完成。

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

2026-07-30 P0 GPU auxiliary pass slice：每个已绑定的 beauty 视图现在还必须由同一 `forgecad-workbench-pbr@1` context 生成一张 `960×640` contact sheet，按固定 tile 顺序包含 silhouette、normal、depth、part-ID、material-ID 五个 `320×320` pass。前端以 offscreen `WebGLRenderTarget` 和同一 camera/scene/renderer 读取 auxiliary pixels，不创建第二 WebGL context；part/material pass 用稳定 hash color 编码，临时材质、overlay 与可见性在 capture 后恢复。Tauri 传输每视图 beauty+auxiliary PNG；Rust 验证两张 PNG 的 IHDR、每图 SHA、固定 auxiliary 尺寸、独立字节上限及总预算，缺失/错误尺寸即不采用 capture；检查拒绝空白或全图单色，但允许合法二值 silhouette/单 zone ID pass。`desktop:u004-workbench-pbr-capture-playwright` 已在真实 browser canvas 产生完整五通道 bundle；这仍只用 Rust fixture，不能替代 packaged 设备证据。Qwen 比较仍仅接收同源 PBR beauty；辅助 pass 为 Rust-bound deterministic evidence，不可被作为视觉相似度通过的替代。

2026-07-30 P0 GPU auxiliary semantic gate：Rust app-server 现在对被采用的每个 auxiliary contact sheet 逐像素提取 `CandidatePbrAuxiliaryQuality@1` 瞬态事实：五个 tile 的前景占比、前景颜色基数和通道范围。所有八个 turntable view 都必须具有五个非空 pass，且该事实参与 UAS@2 的 `gpu_auxiliary_semantics` hard gate 与 render fingerprint；缺失、损坏或视图不完整会 fail-closed、不会触发 geometry patch，也不生成预览/版本。该指标只证明同一 GPU renderer 产出了有几何语义的辅助证据，不能替代 Qwen 相似度、照片级质量或真人评分；合法单材质、单分件的平坦 pass 不因低颜色基数失败。已新增 app-server 正/负单元测试；仍未完成 packaged 设备真 GPU E2E、真实千问比较或未见输入评测。

2026-07-30 P0.1 用户概念图 GPU 路由：已确认 Agent 资产的“生成概念图”在 macOS/Tauri 工作台不再请求 Python 软件光栅接口，而是读取当前已挂载 `production_concept` GLB 的 hash，直接从同一个 Three.js/PBR canvas 采集 `iso/front/side/top` 四张 display-sRGB PNG，其中 `side` 使用工作台登记的 `right` 相机姿态。source hash、PBR GLB lineage、renderer identity 和 `glb_pbr` load state 任一缺失都 fail-closed；GPU 结果不提供未经同源分件验证的爆炸图或旧概念图包，逐张 PNG 仍为只读下载。浏览器兼容和历史接口仍返回 `forgecad-agent-software-raster@1` 诊断视图，但该 renderer 不得进入通用质量验收。通过：`desktop:u004-workbench-pbr-capture-smoke`（含路由回归）、`desktop:u004-workbench-pbr-capture-playwright`、`desktop:u002-universal-author-workbench-smoke`、`desktop:r3-concept-workbench-smoke`、`desktop:typecheck`、`contracts:types:check` 和 `git diff --check`；这不等同真实千问评分、照片级重建或跨类别质量通过。

U004 P1 排序：不新增第二 DSL。VP203/VP204 `ForgeVisualGeometryProgram@2`（profile/extrude/revolve/loft/sweep/boolean/mirror/array/Part/Zone，source-map/readback/单 patch）现已接到 `UniversalAssetSource@2` 与唯一 ActionLoop：`procedural.generic_hard_surface_v1` 只能通过 source hash、同源 PBR capture、VP204 stable node/material patch 与重新 capture 的候选链执行，绝不回退 C111/机械臂。2026-07-30 已补齐同一 DSL 对 restricted worker 已有 `cylinder/capsule/wedge/radial_array/bevel_approx/surface_panel/shell` 的 typed 表达和 fail-closed 参数限制；其中 `shell` 是受限的单盒体闭合薄壁 CSG，不是 `subtract` 别名，厚度按源短边比例封顶并写入真实 feature/readback。随后新增 `GenericHardSurfaceAppearanceCompilation@2`，由 Rust 从 sealed profile/feature/material-zone 选择一至八个真实外观区，逐区生成并校验 normal-relief/roughness/条件式 emissive `SurfaceLayerProgram@1`，并将 plural sealed lowerings 接入现有 production 五通道 PBR GLB worker。照片外观投影仍未烘焙，但其合同现要求 evidence artifact、unobserved texel mask 与已拟合相机属于同一 sealed view，且 artifact 的通道必须匹配 BaseColor/Normal/Roughness/Metallic；unresolved 相机、跨图拼接或将猜测标为 observed 全部拒绝。它不是第二 DSL、不是 Provider 自报材质、不是 UV/多区照片纹理恢复；patch 会重新派生该层。只有该 source-kind 完成真实千问比较、packaged E2E、preview→confirm/version/Snapshot/export 与未见输入评测，才可标为已实现能力；角色/生物/植物等仍等待各自表示，不得借 generic hard-surface 或 C111 模板执行。

2026-07-30 P4 游戏交付真实性：`GameAssetProfile@1` 只能在 UAS@2 planned state 封存 LOD、collision Part、socket 和目标 texel density；它们不是由 1K PBR 或单 mesh 自动推导出的交付事实。`compile_game_asset_lod_delivery` 追加真实 LOD1/2 index accessors 与 `MSFT_lod`，并以 sealed source/profile 重算拓扑、预算和 2% 全资产尺度误差。`derive_game_asset_delivery_bindings` 只从已验证 UAS@2 将 SubjectPart 映射到 GLB `forgecad_feature_node_id`；`compile_game_asset_delivery` 再以该 mapping 写入 off-scene AABB collision meshes 和 socket pivot/forward quaternion nodes，`verify_game_asset_delivery_glb` 重算其 bounds、index、transform 和 lineage。最终 LOD0 的 `GameAssetTexelDensityReadback` 从真实 POSITION/UV0 三角面和嵌入 base-color PNG 的 IHDR 尺寸计算有效 pixels/m，逐 material zone 封存实测值；profile target 只形成 `target_met`，绝不替代测量，`target_met=false` 不可附着 delivery receipt。runtime 先保留 source LOD0 给 PBR/比较，再以 Rust 派生 delivery GLB；确认现在使用专用双工件原子事务保留 source LOD0、delivery GLB 和 interactive preview。`:model.glb`、`:export` 与 `ForgeAssetPackage@2` 都在重新验证同一 source/profile/binding/receipt 后返回 delivery bytes，并将 source hash 与 delivery hash 同时封存。新增 `GameAssetDeliveryRequest@1` 只携带用户 profile、LOD budget 和目标密度；Rust 从实际 executable Part bindings 派生 collision IDs，socket 初始为空，Provider override fail-closed。非游戏 UAS@2 preview 继续使用独立 `NativeUniversalPreviewProvenance@1`，不重用旧 V003/Domain Pack 谱系。C111 smoke 已通过完整 readback；不达质量界限的预算继续返回 `GAME_ASSET_LOD_QUALITY_BUDGET_CONFLICT`。确定性有效 GLB 已完成 source→delivery→preview→confirm→export/package E2E，并通过 receipt 篡改与确认重放幂等；repository 已补 source/delivery CAS 存储漂移拒绝回归。

2026-07-30 P2 受限像素烘焙基础：sidecar worker 保留 `ReferenceUvEvidenceBake@1` 的 declared-rectangle 兼容覆盖，并新增真三角面路径 `ReferenceCameraUvRasterBake@2`。V2 只接收 SHA-256 封存 PNG、明确 retained Design Surface zone、受限的最终 GLB positions/UV0/indices 与 row-major world-to-clip；Worker 用所有 zone 深度 raster，只有可见 target zone 的 source pixels 才落入真实 UV texel，其余程序化 base-color 不变并由 inverse `unobserved_texel_mask` 标记。GLB receipt/readback 现在严格区分 V1 与 `ReferenceCameraUvRasterBakeReceipt@2`，后者绑定 source/camera provenance、matrix hash、triangle count、map/mask hash 与 coverage。`RestrictedGeometryExecutionRequest@1` 继续拒绝无 retained layer、跨/重复 zone 或 render 请求携带此输入；focused direct + compile→同一 GLB/readback 回归共 36 passed。P2.3 已由 Rust 从 exact candidate GLB SHA、最终 compile-readback bounds 和冻结八视图导出 `ProjectionCameraBinding@1`；capture session 固定每视图 binding SHA，app-server 重验 Project/Turn/session 后才返回绑定，工作台以唯一 Three.js renderer 应用它并在上传证据中回传同一 SHA。P2.4 已将 sealed source 闭合在 Rust：`CoreRepository` 只按同 Project evidence ID 读取 immutable `reference` CAS relation，V2 builder 重验 `ReferenceEvidence`、`image/png`、CAS SHA、8 MiB/worker PNG profile、zone ID 与可复验 binding，再生成不持久化的 DTO；跨 Project、byte/hash、zone 或 camera drift 全部拒绝。该链将相机和参考图从 Worker 的外部前提变为候选 GLB/Project 谱系的一部分。尚没有多视图融合、工作台图片投影入口或完整 ActionLoop wiring，故不得视为用户可用照片投影能力。
2026-07-30 U004 P2.6 同区受限多视图融合：同一 retained Material Zone 最多接收两份不同 sealed evidence/camera identity；只有 `ReferenceCameraUvRasterBake@2` 可融合。Worker 对每个视图使用同一最终三角形集合独立 depth raster，按 canonical projection hash 固定排序，对重叠观察 texel 等权平均，对未观察 texel 保留生成底图并取 inverse mask 并集；GLB readback 使用 `ReferenceCameraUvRasterFusionReceipt@3` 封存两份 source/camera/matrix lineage、raster triangle count、fusion count 与覆盖量。Rust/Python 两侧均拒绝第三视图、重复身份和 V1/V2 混合。通过 `test_reference_uv_projection.py`、`test_k003_restricted_geometry_executor.py`、forgecad-core 231 tests、forgecad-app-server 258 tests；这不是任意多图拼接、相机求解、完整工作台入口、真实千问或照片级质量证据。
2026-07-30 U004 P2.7 参考外观回执闭合：Worker 的 `ReferenceCameraUvRasterBakeReceipt@2` / `ReferenceCameraUvRasterFusionReceipt@3` 现在必须先通过桌面 Rust bridge 的原始 `schema_version` 兼容解析，再逐项重验材质区、导入 base-color 的纹理 hash/尺寸、source evidence、camera provenance、world-to-clip hash、融合顺序和 coverage。bridge 只向 app-server 传递受限 hash/count DTO；UAS@2 再将 request/program/final GLB/compile-readback/worker receipt lineage 封存为 `ReferenceAppearanceProjectionReceipt@1`，并把投影回执摘要加入 build ledger projection。缺失、伪造、跨区或 final artifact/readback 不一致均在 geometry candidate 之前 fail-closed。通过 `u004_reference_projection_receipt_missing_fails_closed_without_candidate_state`、U004 source receipt lineage test、desktop bridge readback extraction test、Python camera-raster/fusion 3 tests、234 个 Core tests、app-server U004 13 tests 和 desktop typecheck/build；该切片证明“参考像素确实进入已接受的 PBR artifact 且可追溯”，不证明照片级相似度、真实千问评分、角色/生物表示或跨类别质量。

2026-07-30 P3 受限本地形变与逐部件 Hybrid 执行切片：runtime manifest、JSON Schema、Python semantic validator、Rust G819 validator 和 `ForgeVisualGeometryProgram@2` 同时新增 `lattice_deform`。它不是任意网格编辑：仅接收一个较早 mesh、固定八个 `corner_offsets` 组成 `2×2×2` trilinear cage，偏移逐轴封顶为源 AABB 的 `±25%`，且必须非零；profile、旋转穿透、越界 cage、degenerate bounds/triangle 全部 fail-closed。Worker 不增加三角数、不导入外部 mesh、不执行代码，仍保留原 Part/Zone/material lineage 和 feature readback。`deformable.local_lattice_shell_v1` 已由 Rust capability registry 选择；UAS@2 现可在同一资产中让部分 Part 保持 `procedural.generic_hard_surface_v1`，让明确 Part 使用 lattice，并重验 terminal、offset、计划映射和 component binding。author/build/readback/preview lineage/同源 PBR capture/visual convergence/一次 VP204 patch 均走同一 ActionLoop 路由。随后修复了 Skill manifest 的合同漂移：公开 JSON Schema、生成类型/registry、Rust 上限和 G819 runtime fixture 现在都包含 19 个操作，fixture 真实编译 lattice 变形并验证未知操作 fail-closed。通过 Python deterministic/readback 4 tests、G819 manifest smoke、Core UAS focused、app-server local-hybrid author/build focused、ActionLoop route focused 与 `contracts:types:check`。这仍只开放可审计的硬表面局部混合；不等同任意 mesh、角色、生物、mesh seed 或神经重建，真实千问、packaged E2E、确认版本/导出和跨类别质量仍未运行。

2026-07-31 U004 P3.1 bounded local mesh patch：新增 `local_mesh_patch` runtime/high-level node 与 `mesh_seed.local_patch_v1` capability。它只接收一个已通过 ShapeProgram 审查的较早 mesh，使用归一化 `patch_center`、`patch_radius=0.05–0.4` 和每轴 `patch_offset≤0.2` 的平滑球形影响；三角拓扑、材质、Part/Zone、feature history 和 GLB readback 保持不变，顶点数组、外部 GLB、文件/URL、脚本和任意 mesh 注入全部拒绝。Schema、生成 registry、Python validator/Worker、Rust G819/native executor、VP203 lowering、UAS@2 source binding、author/build/typed-patch route 和 focused fixtures 已同步；G819 fixture 现覆盖 20 个 runtime operation。通过 `test_local_mesh_patch.py` 4 tests、VP203 lowering focused、Core UAS focused、app-server author/build route focused；该能力只证明生成后局部变形切片，不代表通用 mesh seed、导入网格编辑、角色/生物/布料或跨类别质量。

2026-07-30 P4 真实有效 GLB 闭环补齐：新增 desktop bridge focused E2E，使用确定性有效 `production_concept` GLB fixture 证明 source LOD0 与 delivery GLB 不是同一字节：完成 capture、Rust hard gate、单一 delivery preview、confirm、source/delivery 双工件版本、delivery export 和 `ForgeAssetPackage@2`，并验证 receipt 篡改 fail-closed 与重复确认幂等。2026-07-31 又将同一 E2E 改为使用 Rust-bound `GameAssetDeliveryRequest@1`，不再由 Provider 自报 `GameAssetProfile@1`；工作台 selector、StartAgentTurn wire、digest/idempotency 和 Product Tool binding 已接通，Provider override 负测通过。随后补充 source/delivery 两个 CAS 对象被篡改后的 repository readback 拒绝回归，并验证恢复原字节后 bundle 可读。该证据只证明本地受限 hard-surface/game-delivery 工程闭环，不证明真实千问、packaged loader 或视觉 4/5。

剩余退出条件：

1. 为 local-hybrid 补齐真实工作台与 packaged E2E、完整 confirm/version/export 与真实千问比较证据；非游戏 UAS@2 的确认 provenance 已接入现有单 GLB 事务，但必须用真实 GLB 端到端覆盖；游戏 delivery 的有效 fixture 闭环、bounded 用户 profile intent、source/delivery CAS 存储漂移拒绝和已确认资产 ChangeSet 双工件 repository/bridge wiring 已通过，且 native compatibility 的 `:preview → :preview.glb → :confirm` 全路径已通过 source/delivery 保留与 material ChangeSet 回归；仍需 packaged loader、真实千问比较和用户质量证据；其 source、参数、预算和 readback Schema 已冻结并由 Rust capability registry 选择，DeepSeek/千问/前端不能自报 executable；
2. 每个表示绑定 request/profile/feature/plan/source、CAS、Part/Zone、GLB/PBR、固定视图和未观察区域；
3. 建立角色/生物/植物/布料等所需的受限曲面、骨架/形变与局部细节算子；任意代码、动态文件/URL 和无界生成继续拒绝；
4. 千问固定多视图比较与 Rust hard gate 通过后，候选才进入唯一 `SingleResultDecision`、preview→confirm、不可变版本、Snapshot 和导出；
5. Provider 取消/超时/非法输出与本地编译失败保持零永久副作用；不存在的表示继续 typed limitation；
6. 八类真实未见输入证明身份、轮廓、结构和材质净提升，失败不回退模板；完整 U004、U002/U003、工作台、G1–G7、文档和发布基线 Gate 通过。

2026-07-30 U004 P1.1 特征驱动 Appearance Compiler：`compile_generic_hard_surface_appearance` 现将 Rust sealed `VisualFeatureRequirement.description`、显著性和目标通道与部件材质特征合并为确定性 `SurfaceLayerProgram@1` 选择输入。边缘/中心/轮廓证据会选择覆盖范围，角向/人字/三角证据选择 `chevron_relief`，蜂窝/六边形/网格证据选择 `microgrid`，磨损/拉丝证据选择 `edge_wear`/`linear_brush`，指示/传感/状态/点阵证据选择 `panel_indicator`/`dot_array`，且只在已声明 emissive 通道时生成发光层；显著性控制受限强度，仍最多八个真实 Part/Material Zone。Rust focused test 和 Python Worker PBR test 已验证新 motif 会改变实际 normal/metallic-roughness/emissive 输出，而非只改变合同文本。该切片提升同一受限编译器的外观自由度，不证明照片纹理投影、真实千问评分、角色/生物表示或跨类别质量。

2026-07-30 U004 P1.2 可见证据驱动 decal lowering：Appearance Compiler 现在只从 `Observed` 或 `Inferred` 且声明 `BaseColor` 通道的 sealed 特征选择最多一个受限 decal；warning/hex/chevron/label 语言分别映射到 `warning_stripe`、`hex_badge`、`chevron_mark`、`panel_label`，颜色、固定文本 token、UV 锚点、尺寸和透明度均由 Rust bounded vocabulary 确定。`Hidden`/`Conflicting` 特征仍保留在 VisualFeatureContract，但不能改变可见 PBR；对称/环形证据可选择受限 mirror/radial UV symmetry。Core 已验证可见 hex badge 和隐藏证据零 decal，Python Worker 已验证 decal 实际改变 base-color PNG。该切片不是任意文字渲染、贴图上传或照片投影。
2026-07-30 U004 P1.3 可见特征驱动 vector path lowering：Appearance Compiler 现在将 `Observed`/`Inferred` 且声明 BaseColor/Normal/Roughness/Metallic 视觉通道的 seam、panel、groove、slot、trim、contour、edge 等语言编译为最多一个 Rust-owned `VectorPath`。路径仅是 retained PBR 纹理层，不是 SVG、CAD 草图或几何边；Worker 已将其实际写入 base-color、metallic-roughness 和 occlusion 输出。Core 验证可见 panel seam 产生路径、隐藏特征不产生可见路径，Python Worker 验证移除路径会改变实际 PBR bytes。该切片仍不实现多视图融合、照片投影、任意曲线或角色/生物表示。

2026-07-30 U004 P1.4 六面 surface-panel lowering：同一受限 `surface_panel` 现在支持 `±X/±Y/±Z` 六个局部轴向面。Rust VP203、app-server validator、Python ShapeProgram validator 和 Worker 使用同一 face-axis 合同；面内位置允许局部偏移，法向位置由源盒体面和 panel 厚度自动计算，斜轴、法向偏移、越界 panel 和不匹配源仍 fail-closed。Worker 真实编译侧装甲/检修板/垂直饰面 GLB，保留 Part/Zone/material/readback lineage；通过 Python ShapeProgram/static-transform 30 tests、app-server Rust focused validator 和 forgecad-core VP203 12 tests。该切片扩展硬表面表达力，不是任意曲面投影、自由 mesh 编辑或角色/生物表示。

2026-07-30 U004 P1.5 倒角盒体 shell lowering：受限 `shell` 现在可接收直接 `box` 或已验证的 `bevel_approx` 源，生成带真实圆角边缘的闭合薄壁壳体。Rust VP203、app-server validator、Python ShapeProgram validator 和 Worker 共享 `radius/segments` 与内腔净空合同；倒角半径不超过源盒体 X/Z 最短边的 `25%`，且必须为 shell 厚度留下至少 `2r` 的内腔余量。Worker 继续通过本地 Manifold CSG 编译真实 GLB/readback，不开放任意 offset、任意 CSG 或网格输入；无效半径、段数、厚度、源类型和净空全部 fail-closed。通过 beveled-shell deterministic/readback、Rust VP203 和 native executor focused tests；该切片提升科幻装甲/设备外壳/护罩的边缘质量，不代表通用 CAD shell、角色/生物或跨类别质量完成。

2026-07-30 U004 P1.6 受限 face-groove lowering：高层 `Groove` 节点接受直接 `box`/`bevel_approx` 源、六个轴向面、二维面内尺寸、面内偏移和 bounded 深度；Rust 将其确定性展开为带 `0.1mm` 外溢余量的 cutter box 与 `subtract`，Worker 使用本地 Manifold CSG 生成真实浅凹结构。源面外、法向偏移、越界尺寸、深度超过源法向尺寸 `25%` 或图预算不足均 fail-closed；它不是任意 boolean、SVG、自由 mesh 或 provider 代码。schema/生成 registry、VP204 node lineage、Core lowering、Python deterministic/readback 与 G819 manifest 已同步；该切片只提升装甲面板线、散热槽和检修凹槽的几何可读性，不能改写为跨类别质量已完成。

2026-07-30 U004 P1.7 高层几何合同闭合与静态姿态：`ForgeVisualGeometryProgram@2` 的公开 JSON Schema、生成类型/registry、Rust high-level lowering 与现有受限 Worker 现在共同声明全部 19 个可执行操作：基础体、profile/extrude/revolve/loft/sweep、mirror/array/radial_array、bevel/surface_panel/groove/shell/lattice_deform、union/subtract、Part/Material Zone。新增 operation-coverage fixture 逐项覆盖 Schema；高层基础体和 profile 节点暴露有界静态 Euler rotation（每轴 `[-π,π]`），零旋转省略序列化以保持旧程序 hash/identity，非有限或越界值在 Rust lowering 前 fail-closed。通过 `contracts:types:check`、`agent:vp203-high-level-geometry-gate`（含 15 个 VP203 Rust tests、3 个 GLB/readback fixture 和 19-op schema coverage）与 rotation focused test。该切片关闭 Provider 合法输出被 Schema 拒绝的合同漂移并提升武器/装甲/载具部件姿态自由度；仍只执行已登记的受限 hard-surface/local-lattice，不开放任意 mesh、角色/生物表示、照片重建或跨类别质量。

2026-07-30 U004 P2.8 Provider 合同投影闭合：`author_universal_asset` 的 Provider-facing schema 现在由已登记的 `UniversalAuthorOutcome@1`、`UniversalAuthorRequest@1`、`SubjectProfile@1`、`VisualFeatureContract@1`、`RepresentationPlan@1` 和 `RepresentationLimitation@1` 公共合同内联生成，不再只向 DeepSeek 暴露一个无约束 `object`。Rust 同时把只读 `RepresentationCapabilityManifest@1`、精确 manifest hash、available capability 和 unavailable 分支投影到 Provider context，使模型能知道 `procedural.generic_hard_surface_v1`、`deformable.local_lattice_shell_v1` 与 `mesh_seed.generic_v1` 的真实状态；Rust typed deserialization、hash、部件/特征引用和 executable/limitation 语义仍是最终真值。补齐 Product Tool validator 的小写 64 位 SHA-256 模式，避免公开合同启用后误拒绝合法 request/profile/plan hash。通过 product_tools 103 项、app-server 完整 266 项、U004 candidate PBR contract Gate、contracts types generate/check 与 desktop typecheck；没有调用 DeepSeek、千问、Fal 或任何付费 Provider。该切片只减少 Provider 合同猜测和返工，不新增角色/生物/任意 mesh 表示，也不证明照片级质量。

2026-07-31 U004 P2.9 universal image author transport：桌面层新增唯一 `buildAgentTurnRequestPayload`，将工作台选中的 sealed image 以 evidence ID、用户角色和 view hint 发送到 Rust `author_context`，并保留完整视觉图供 Rust 归一化为 `VisualEvidenceGraph@2`；客户端不传 evidence hash、Project、Turn、Snapshot 或 capability 真值。Universal image Turn 不再携带旧 `multimodal_context`，协议对双来源请求 fail-closed；旧字段仍可被历史调用单独兼容。新增 payload smoke 覆盖多图、视图提示、单一来源和 text-only 空上下文；通过 `desktop:u002-universal-author-workbench-smoke`、F026 smoke、desktop typecheck、app-server-protocol 42 tests 和 diff check。该任务只关闭桌面 transport gap，不新增几何能力，不代表完整 ActionLoop 像素投影、真实 Provider、packaged GPU、照片级或跨类别质量完成。

2026-07-31 U004 P2.10 相机拟合门控与二次参考外观编译：首次 UAS@2 编译不再在 unresolved/default camera 下烘焙参考图；Rust 先接收同源 GPU silhouette，拟合后按最终 view 重编译 UV/PBR/GLB/readback，并使工作台重新 capture。相机拟合不通过时不进入授权、Qwen、preview 或版本。Rust reopen、candidate PBR、valid/Hybrid/limitation bridge focused tests 通过；状态仍为 U004 `in_progress`，不解锁 U005。

2026-07-31 U004 P2.11 两阶段实际桥接与 PBR 材质完整性门：valid sealed-image fixture 现在通过 `AppServerBridge::resume_candidate_pbr_capture` 验证首次返回 `capture_required`，新 GLB 重新 capture 后返回 `authorization_required`，而不是直接调用 executor。`readWorkbenchPbrViewportIdentity` 还拒绝缺失完整五通道、嵌入纹理不足、错误色彩空间或错误采样的候选。通过 valid bridge focused、U004 workbench smoke、GPU/PBR Playwright、desktop typecheck；仍不解锁 U005。

2026-07-31 U004 P2.12 final GLB pixel truth：Rust bridge 不再只信任 Worker 的 texture manifest/receipt；它从 exact final GLB 独立解析 `material → baseColorTexture → image → bufferView`，重新计算 embedded base-color PNG 与 unobserved mask 的 SHA-256、字节数和 PNG 尺寸。核心 `verify_forgecad_glb` 仅允许 base-color 使用 `imported_reference/unknown`，metallic-roughness、normal、occlusion、emissive 仍保持 builtin contract。U004 fake geometry fixture 现在嵌入与 receipt 完全一致的真实 PNG，`u004_universal_image_valid_glb_preview_confirm_and_export_round_trip` 和核心 stale-PBR 回归通过；这关闭了“回执说已投影、最终 GLB 仍是内置底图”的假阳性。未调用 Provider、未 commit/merge/push；不证明照片级相似度、真实千问、packaged GPU、角色/生物表示或跨类别真人质量。

2026-07-31 U004 P2.13 Rust-owned contour profile fit：candidate PBR capture 从同一 auxiliary silhouette pass 派生 16 个前景占用采样；若 Rust 能读取同 Project、同 hash 的 sealed image bytes，参考侧也派生同样 profile。`SilhouetteFit` 现在同时检查 bounds IoU、中心误差和 bounded profile error；profile 缺失时保留 legacy fixture 的 bounds-only 兼容上限，profile 长度/值非法或轮廓明显冲突则 fail-closed。通过 core profile fit negative/compatibility tests、app-server camera-fit regression 和 cargo check；未调用 Provider、未 commit/merge/push。该切片不证明照片级质量、真实千问、packaged GPU、未见输入或跨类别 4/5。
2026-07-31 U004 P2.14 Rust-owned reference/candidate visual metrics：新增 `RustReferenceVisualMetrics@1` transient summary；Rust 对 exact sealed reference 与同一 GPU/PBR capture 的候选视图计算 profile error、bounds IoU、颜色桶、亮度和边缘密度指标，选出确定性最佳视图；仅在摘要可用且明显偏差时将 failure code 合并到 `VisualReferenceConvergenceEvidence`，旧最小 PNG 保持 `not_available` 兼容。完成 focused native test 与 app-server cargo check；不等于真实 Provider、照片级相似度、packaged GPU 或跨类别质量。

2026-07-31 U004 P2.15 reference-conditioned surface appearance binding：为 generic hard-surface Appearance Compiler 增加 Rust-owned `ReferenceSurfaceAppearanceBinding@1`。它从同 Project、同 semantic hash 的 sealed image 读取 Rust 派生低维 surface facts，并新增 foreground color buckets；编译 hash 封存 evidence/facts/bounded fallback tokens，程序化、local lattice、Hybrid、local mesh patch 及 VP204 typed patch 都沿同一 source 重编译。focused Core/app-server、生成合同和 hash-drift negative tests 通过；状态为 `部分实现`，不代表照片级材质恢复、真实 Qwen 或跨类别 4/5。

2026-07-31 U004 P2.16 reference fallback scope：修正 P2.15 的 zone 作用域。Rust 现在只允许参考图低维颜色/finish/roughness fallback 进入外壳、装甲、外部面板等兼容 reviewed base material；structural frame、accent trim、rubber、glass、emissive、signal/warning 等特殊或内部语义不会被全局参考色覆盖。显式 feature/material 语义仍优先，并补充 black/gray 到 graphite/gunmetal 的显式词映射。新增 Core 负向 scope test；无 Schema 版本变化、无像素/自由 RGB/Provider 自报材质。状态为 `部分实现`，仍不代表照片级材质恢复、真实 Qwen 或跨类别 4/5。
2026-07-31 U004 P2.17 exact observed feature-to-zone appearance scope：参考 surface facts 现在还必须命中 Rust 派生的 observed、appearance-bearing feature region 到同一 Subject Part/Material Zone 的 exact binding；同一 Part 下没有 observed region 的 sibling zone 不再继承整图颜色/finish/roughness。复用 `ReferenceAppearanceBinding@1`，无 Schema 版本变化；Core UAS 7 项 focused tests 通过。该切片只减少多区串色和无证据外观推断，不代表照片级材质恢复、真实 Qwen、packaged GPU 或跨类别 4/5。

2026-08-01 U004 native visual-exterior route regression：补齐 generic visual exterior 的真实 author→UAS@2→native geometry→GLB/readback→capture-pending 回归，以及同 route 的单次 visual repair resume。`u004_generic_visual_exterior_builds_uas_v2_then_requires_desktop_pbr_capture` 与 `generic_visual_exterior_capture_resume_keeps_visual_repair_route` 通过；app-server 全量 281 passed / 0 failed。该切片只证明非机械臂类别可以进入当前受限执行链并诚实停在同 renderer PBR capture，不解锁真实 Provider、packaged GPU、照片级/跨类别质量或 U005。

2026-07-31 FGC-P002 本机 packaged Alpha 重建：`desktop:packaged-sidecar-build`、`release:packaging-readiness-smoke`、`desktop:packaged-sidecar-alpha-smoke`、`desktop:packaged-rust-ownership-smoke`、真实 LaunchServices `desktop:packaged-tauri-alpha-smoke`、K002 和 K003 packaged native smoke 均通过；真实 arm64 frozen sidecar、Rust-owned K001/K002/K003 状态、临时 Library、受限几何归属、GLB/readback/render package 和重启语义 hash 均有本轮证据。K003 健康校验已与当前 Rust supervisor 的动态归属字段对齐并保持严格字段集合/格式检查。P002 仍不等于签名、公证、安装/升级、跨平台 sidecar、生产视觉质量或正式发布。

2026-07-30 U004 P0.2 通用 sealed 图片 bridge 闭环：新增 `u004_universal_image_valid_glb_preview_confirm_and_export_round_trip`，用真实 CAS 封存的 `pack_unclassified` PNG、Rust 派生 `UniversalAuthorRequest@1`/`VisualEvidenceGraph@2`、UAS@2 generic hard-surface candidate 和同一 renderer 的八视图+五通道辅助 capture，绑定一次性千问兼容授权后完成 `evaluate_candidate`→`prepare_candidate_preview`→compat confirm→UAS@2 版本化→`:export`；重放 confirm 保持同一 head，且确认产物没有旧 `ForgeVisualProgram`/C111 回退。该测试使用本地确定性 `qwen3-vl-plus` 兼容 fixture，`network_call_made=false`，没有调用真实千问或任何收费 Provider。另新增 `u004_universal_limitation_bridge_has_zero_geometry_or_version_side_effects`，验证 unavailable `mesh_seed.generic_v1` 只产生 typed limitation，worker、preview、Snapshot、版本和导出计数均为零。Gate 固定为 `desktop:u004-universal-image-bridge-e2e`；它证明 Rust bridge 和既有确认事务的工程闭环，不证明真实千问相似度、照片级质量、packaged GPU、角色/生物表示或跨类别质量。

2026-07-30 U004 P0.3 通用图片主路径协议闭合：工作台参考图不再同时提交旧 `multimodal_context` 与 Universal `author_context`，避免 Universal candidate 在比较阶段被错误选为 Legacy source；Rust `ValidatedUniversalAuthorContext` 在 Provider 生成 `SubjectProfile@1` 后，将只读 `VisualEvidenceGraph@1` 按 macro/meso/micro 投影为绑定 request/profile 的 `VisualEvidenceGraph@2`，无证据特征保持 hidden/conflicting。候选仍必须通过同一 renderer 的八视图后才显示一次千问授权卡，授权前无网络、preview 或版本副作用。通过 `cargo test -p forgecad-app-server universal_author_context::tests --lib`（3/3）、`cargo check -p forgecad-app-server`、`desktop:typecheck` 和 `git diff --check`；真实 DeepSeek/千问、packaged GPU、未见输入、照片级和跨类别质量仍 NOT RUN。
2026-07-31 U004 P0.3 GPU render provenance seal：Rust-issued `CandidatePbrCaptureSession@1` 封存工作台环境 ID/hash、固定 render manifest、sRGB 和 ACES Filmic；Tauri bridge 要求每张 beauty/auxiliary capture 回传相同身份，Core submission/evidence 与 Universal `VisualReferenceRenderContract@1` 对环境、manifest、renderer 和色彩契约逐项重验。环境漂移、缺失或伪造 fail-closed。补齐 legacy 软件栅格 comparison fixture 的显式 `candidate_render_contract: None` 后，`desktop:u004-universal-image-bridge-e2e`、Workbench smoke、真实浏览器 GPU/PBR 五 pass Playwright、完整 `agent:u004-candidate-pbr-capture-contract-gate`、contracts、desktop typecheck、文档/安全/完整性 Gate 均通过。该切片不证明真实 DeepSeek/千问、packaged GPU、未见输入、照片级或跨类别质量。
2026-07-31 U004 P0.4 native concept-render entry seal：macOS/Tauri 概念图入口在调用 `captureWorkbenchPbrConceptViews` 前即验证当前挂载视口的 `forgecad-workbench-pbr@1`、`glb_pbr` ready 和 exact source GLB hash；旧 `forgecad-agent-software-raster@1`、ShapeProgram fallback 或未完成加载不会进入概念图/质量证据路径。新增 loader smoke 覆盖旧 renderer 早拒绝，U004 工作台 PBR smoke 与 desktop typecheck 通过。浏览器软件光栅仍仅作为显式诊断兼容路径；该切片不证明真实千问、packaged GPU、照片级或跨类别视觉质量。
2026-07-31 U004 P0.5 packaged GPU/PBR evidence contract：为既有 `desktop:c111b-packaged-agent-webgl` 增加严格 packaged renderer facts 与 auxiliary pass facts：Rust 只接受 `forgecad-workbench-pbr@1`、固定 manifest/environment、sRGB/ACES、`pbr_texture_count >= 5`；八个 capture 的 `960×640` 五 pass auxiliary PNG 由 Tauri capture command 与 beauty PNG 一起上传，Rust 读取 IHDR、计算 SHA-256、写入受限 `.auxiliary.png` 工件并重验 dimensions/pass IDs/hash，禁止仅信任 WebView 自报。开发 build、Tauri app rebuild、logic smoke、Rust focused test、PBR smoke、typecheck 和 contracts 通过；真实 LaunchServices QA 在本轮被锁屏前置条件阻断（`C111B_PACKAGED_SCREEN_LOCKED`），不能把逻辑/编译通过写成 packaged GPU 通过，也不解锁 U005。

2026-07-30 U004 P4.1 local hard-surface Hybrid 图片闭环：新增 sealed PNG + pack_unclassified 的 Hybrid bridge fixture。单一 UAS@2 资产同时包含 procedural.generic_hard_surface_v1 主壳与 deformable.local_lattice_shell_v1 饰条；Rust 重算两份 Part/Material Zone、固定 lattice terminal/offset、Appearance Compiler 与同一 renderer 八视图/五通道 capture。授权一次性 Qwen-compatible fixture 后完成 evaluate→preview→confirm→UAS@2 version→export，确认重放保持同一 head，persisted source 保持 Hybrid 且没有 legacy ForgeVisualProgram/C111 fallback；隐藏形变特征不会进入视觉比较 claim。测试纳入 desktop:u004-universal-image-bridge-e2e，fixture network_call_made=false，没有调用真实 DeepSeek、千问、Fal 或任何付费 Provider。该证据只证明本地受限 hard-surface Hybrid 的工程闭环，不证明 packaged GPU、真实千问质量、任意 mesh、角色/生物表示或跨类别真人 4/5。

2026-08-01 U004 在线 author 合同可执行性加固：`ValidatedUniversalAuthorContext::provider_projection` 新增只读 `geometry_authoring_playbook`，把 Rust 已登记的 `ForgeVisualGeometryProgram@2` 合法组合链、profile/section/sweep/loft 约束、surface-panel/groove 局部坐标、disjoint output graph、最小有效 branch 与 macro→meso→micro 质量优先级投影给 DeepSeek。同步校验 exact capability manifest/hash 与 sealed evidence ledger；上下文 3/3、Product Tool 3/3、recovery 2/2、U002 contract Gate、app-server 全量 281、core 全量 245、桌面/Tauri/build 和安全文档 Gate 通过。该切片改善在线首轮合同通过率，不放宽 Rust validator，不恢复 C111/机械臂 fallback，也不证明真实 DeepSeek→GLB、千问比较、照片级或跨类别质量。

2026-08-01 U004 逐部件程序化能力组合：修复此前把多个 capability ID 一律判为 `UNIVERSAL_EXECUTABLE_CAPABILITY_MIXED` 的限制。若所有部件都是已实现的程序化 visual capability，`generic_hard_surface` 与 `generic_visual_exterior` 可以在同一 `ForgeVisualGeometryProgram@2` 中按 part 组合；含 visual exterior 时 domain 固定为 `generic_visual_exterior`，Rust 仍只创建一个 `UniversalAssetSource@2` 并沿同一 build/readback/capture 路径继续。新增 Core composition test 与 native `u004_mixed_procedural_parts_keep_one_universal_source_route`；不改变机械臂/C111 隔离和 unavailable representation 边界。

2026-08-01 U004 工作台状态与真实回归收口：空项目进阶模式、首个 ActiveDesignSnapshot hydration、Agent-first 旧 Inspector 遮挡和已确认资产撤销/重做的 Snapshot/ETag race 已修复；R3 smoke 清理旧 UI 断言并在当前开放类别工作台上再次通过。最终 Core 243、app-server 281、U002 contract、G1–G7、U002/PBR/R3 desktop、typecheck/build、Tauri check/app bundle、docs/integrity/safety/secrets/provider-policy Gate 全部通过。该条只记录状态与回归质量，不改变 U004 的 `in_progress`、未解锁 packaged DeepSeek/Qwen 真实验收和跨类别视觉质量阻断。

2026-08-01 U004 类别开放 capability 前置条件修复：generic visual exterior capability 删除了对 Provider 自报 `visual_exterior` 标签的依赖；新增 Core `u004_generic_visual_exterior_accepts_open_category_without_provider_trait` 与 native `u004_generic_visual_exterior_author_uses_universal_source_not_arm_fallback`（animal/quadruped profile）回归。Core 245、app-server 281、U002 contract Gate 与合同生成检查通过；不改变 typed limitation、一次 author/一次 patch、UAS@2 和真实 Provider/真人质量阻断。

2026-08-01 U004 运行时类别开放提示修复：主 `FORGECAD_NATIVE_SYSTEM_PROMPT` 曾在 capability 已开放后仍把角色、生物、植物、家具、建筑和环境全部指向 limitation，导致在线 Provider 在 author 前主动放弃非机器人对象。现已广告 `robotic_arm`、`generic_hard_surface` 与面向任意对象可见非功能外观的 `generic_visual_exterior` 三条当前路线；对象身份保留在 SubjectProfile，不要求 Provider 自报 `visual_exterior`，只有未实现的 deformable/mesh-seed 等表示返回 limitation。新增 native runtime policy regression；app-server 281/281、contracts/docs/integrity/safety/secrets/agent/provider-policy/diff Gate 通过。该修复解除提示层阻断，不宣称真实 DeepSeek GLB、照片级或跨类别质量完成。
2026-08-01 U004 在线 author 几何合同窄修复：真实 DeepSeek 猫 author 已通过开放类别/SubjectProfile 路由但连续暴露 `PROFILE_WINDING_OR_DEGENERATE`、`SECTION_SET_INVALID` 和 `SECTION_RESAMPLE_MISMATCH`。新增 Rust ActionLoop 一次性恢复文案，明确闭合逆时针 profile、严格 section 顺序/端盖/统一采样计数；author playbook 对 organic/animal generic_visual_exterior 优先 capsule/box/cylinder，避免不必要 loft。app-server 全量 282/282、U002 workbench smoke、desktop typecheck、contracts/OpenAPI、Tauri bundle 和 diff check 通过。真实第三次请求在自动锁屏时仍 running，尚无 GLB/readback 成功证据；不解锁 U005 或跨类别质量门。
2026-08-01 U004 SubjectProfile feature 合同窄修复：真实猫 author 的最后稳定失败为 `SUBJECT_FEATURE_INVALID`。新增一次性 recovery，要求 feature_id 唯一、feature.part_id 原样引用 SubjectProfile.parts，并明确 SubjectProfile 与 VFC `affected_part_ids` 不可混用；app-server 282/282、U002 smoke、desktop typecheck、contracts/OpenAPI、最新 Tauri bundle 和 diff check 通过。未对新 bundle 再发起付费请求；真实 DeepSeek→GLB/readback 仍 NOT RUN/未通过。
2026-08-01 U004 SubjectProfile author 提示边界加固：为 `SUBJECT_FEATURE_INVALID` 增加更具体的只读 author projection 规则，明确扁平特征字段、唯一 feature ID、声明部件逐字引用和 macro/meso/micro 最小样例；禁止把 VFC `affected_part_ids` 或 RepresentationPlan `covered_feature_ids` 混入 SubjectProfile。通过 app-server projection focused test、本机跨类别 provider 1/1、U004 PBR capture smoke、desktop typecheck、contracts check、packaged QA logic smoke、diff check 和最新 Tauri bundle。真实 DeepSeek 下一次重试因 macOS 锁屏尚未执行，U004 继续 in_progress。

2026-08-01 U004 工作台入口类别开放清理：移除主路径的机器人/车辆/无人机模板暗示，补充动物、角色/生物、家具/产品、建筑/环境、游戏道具和混合对象示例；GLB 导入停止按文件名猜 Domain Pack，默认封存为 `pack_unclassified`；`generic_visual_exterior` 现在可消费通用内置 PBR 材质目录。通过 F002、F026、U002 workbench smoke、desktop typecheck、Tauri bundle 和 diff check。只修复入口与材质目录可达性，不改变 Rust validator 或 legacy fixture；真实 DeepSeek→GLB/readback 仍待解锁后重跑。

## 7. 冻结回归边界

- C111B/E005 fixture、预算、readback 和时间证据继续回归，但不能写成通用质量；
- M108A 只证明 production 工件，真人视觉质量归 U005；
- Weapon/Concept 兼容任务只按 `COMPATIBILITY_MIGRATION.md` 推进；
- 未经用户精确授权，不运行真实 DeepSeek/千问付费调用、30 题批次或真人评审；第三方 AI Provider 永久禁止。

2026-07-31 U004 P1.8 bounded appearance color semantics：为 `SurfaceLayerProgram@1`/`RetainedSurfaceLayers` 增加可选 `base_color_token`，Rust 只接受六个固定颜色语义；generic hard-surface Appearance Compiler 从 sealed feature/material text 选择 token，Python retained five-channel PBR bake 将其写入实际 base-color，GLB/readback 保留同一 hash lineage。旧 JSON 无字段兼容，未知 token/自由 RGB fail-closed。Rust Core UAS 6、native executor 1、Python 35、contracts generate/check 通过。该切片是外观组合增量，不是照片级、角色/生物或跨类别质量退出证据。

2026-07-31 U004 P1.9 bounded surface finish semantics：新增 `surface_finish_token`，由 Rust 从 sealed feature/material text 选择八个固定 finish 之一，Python retained PBR bake 实际改变 metallic/roughness 通道；schema/generated types、Rust/Python/native executor 测试通过，未知 finish/free scalar fail-closed。该切片让材质区不仅有颜色，还能区分拉丝金属、抛光金属、陶瓷涂层、哑光涂层、橡胶、玻璃和发光饰条；仍不证明照片级、真实千问或跨类别质量。

2026-08-01 U004 在线 universal-author 合同失败修复：真实 DeepSeek 记录确认最新请求走 `author_universal_asset`，工作台旧 `plan_complete_concept` 卡属于兼容展示误判。针对 provider 输出的 VFC requirements 不完整、RepresentationPlan parts 不完整、part/feature 双向关系不一致和 `sphere` 等未登记 geometry kind，DeepSeek projection 现在明确 exact closed sets、逐部件/逐特征一一对应、合法 geometry vocabulary 和圆形外观替代写法；ActionLoop 对这些失败码各提供一次窄 typed recovery，前端只在真实旧工具 item 存在时显示兼容规划证据。通过 app-server full 282、U002 contract、desktop workbench smoke、contracts/docs/integrity/safety/secrets/agent/provider-policy、desktop build/Tauri bundle。真实联网 DeepSeek→GLB/readback 仍待解锁 macOS 后重跑；没有放宽 Rust validator、没有机械臂/C111 fallback、没有调用 Fal/Hunyuan/第三方 Mesh、未 commit/merge/push。

2026-08-01 U004 Rust universal-author canonicalization：真实 DeepSeek 猫输出已经证明类别理解与 `generic_visual_exterior` 计划可达，剩余失败集中在重复镜像 feature ID、loft resample/cap/顺序 metadata、镜像祖先图 fan-out 和低报预算。`author_universal_asset` 现由 Rust 在 lineage 绑定前做有界 canonicalization：镜像 feature/VFC 按具体 part 拆分并重绑 plan；明显 clockwise profile 反转；有限唯一 section positions 排序并规范 cap/resample；安全时为跨输出 mirror 克隆仅几何祖先；预算下限按实际结构提升但不超过 reviewed ceilings，triangle estimate 仍是最终硬门。非法/不确定结构继续 fail-closed，不生成 C111/机械臂替代。新增 2 个 focused tests；`cargo check -p forgecad-app-server`、app-server 全量 284/284 通过。尚未解锁工作台重建并复测真实 DeepSeek→GLB/readback，U004 仍 `in_progress`，U005 不解锁。

2026-08-01 U004 验证补充：本轮静态与本地合同 Gate 继续通过；最新 R3 smoke 在参考 GLB 导入后找不到当前 UI 的“分件候选/导入参考模型 v1”节点，桥接 E2E 重链超过 4 分钟无输出后终止，均保留为未通过/未运行，不修改任务状态。U004 仍需解锁 macOS 后用最新 `.app` 对猫、建筑或另一类别完成真实 DeepSeek→GLB/readback/capture；在此之前不得把类别开放能力写成跨类别高质量完成。

2026-08-01 U004 兼容入口修复：Python test oracle 的 GLB 导入现接受 Rust 规定的 `pack_unclassified`，并将 R3 smoke 改为显式等待导入响应。`desktop:r3-concept-workbench-smoke` 与 `agent:unit`（183 passed）通过；该修复只同步兼容测试入口，不增加 Domain allowlist 或模板回退。U004 仍 `in_progress`，真实 DeepSeek→GLB/readback/capture、千问比较和跨类别质量门仍待解锁后验证。
