# ForgeCAD 当前能力—Gate 矩阵

版本：2026-07-29
状态：当前能力与阻断的唯一总表

| 能力 | 当前状态 | 当前证据/Gate | 未完成边界 |
| --- | --- | --- | --- |
| Rust-owned Agent 与产品状态 | 已实现（Alpha） | K001–K003；`agent:k001-app-server-smoke`、`k003:layered-gate` | 正式签名、跨平台安装、广泛并发 |
| `ActiveDesignSnapshot@1` 单一真值 | 已实现（Alpha） | FGC-S001–S008；`agent:s1-active-design-snapshot-smoke`、`desktop:r3-concept-workbench-smoke` | legacy UI 完全退出、多客户端压力 |
| 唯一结果、preview→confirm、不可变版本 | 已实现（Alpha） | V003、F026、PV005；工作台与连续资产 Gate | 普遍视觉质量未证明 |
| 单 renderer `docked | focus` | 已实现（Alpha） | F026 规格；`desktop:f026-codex-workbench-smoke` | 原生长时间资源测试 |
| 受限程序化几何与 CSG | 已实现（Alpha） | G801–G826、Manifold Python、GLB feature history/readback Gate | 不是 B-Rep、自由曲面或任意代码执行 |
| GLB/PBR/Material Zone/readback/export | 已实现（Alpha） | M108A、G826、A005、PV005/PV008 | 自动事实门不等于真人外观质量 |
| 参考证据与多模态合同 | 部分实现 | R007A/B、PV006A/B；exact-lineage 工程 Gate | 当前 Project 真实图片组合、视觉相似度、跨类别正式 run |
| `ForgeVisualProgram@2` / 1+1 | 部分实现 | VP201–VP204、E005-R1/R2/R3 Core Gate | 通用 Assembly/Surface、真实四模态、完整时间和真人质量 |
| 当前机械臂生成路径 | 部分实现 | 真实 DeepSeek 文本→Rust→GLB→preview/confirm/Snapshot/export 工程闭环 | 画面低于目标参考；sealed comparison 和独立真人 4/5 未完成 |
| 类别开放通用入口 | 部分实现 | U002；`agent:u002-universal-author-contract-gate`、`desktop:u002-universal-author-workbench-smoke`、U004 focused author/build/capture/VP204 patch tests，以及 `u004_compiled_universal_source_creates_confirmable_preview_without_legacy_plan` | 机械臂与受限 `procedural.generic_hard_surface_v1` 可进入工程候选链；非游戏 UAS@2 preview 现在可用 Rust-validated universal provenance 进入既有确认事务，但尚未完成真实千问、packaged E2E、完整通用 confirm/version/export；角色、生物、植物等仍 typed limitation |
| `UniversalAssetSource` 与通用外观绑定 | 已实现（当前程序化切片） | U003；`agent:u003-universal-asset-source-gate`；Rust author→build→preview/版本 exact-lineage；U004 P2.3 `ProjectionCameraBinding@1` 将 candidate GLB/readback bounds/turntable view 绑定到同一 renderer capture session；P2.4 `CoreRepository` 仅从同 Project sealed PNG CAS relation 构造并重验 `ReferenceCameraUvRasterBake@2`；UAS@2 non-game preview 有独立 universal provenance，确认时将 exact source/hash 写入同一 AssemblyGraph；游戏 delivery 已有双工件原子持久化和 receipt-verified export/package foundation | 没有多视图融合、用户图片投影入口、完整 ActionLoop pixel-bake wiring 或照片 PBR 恢复；游戏 delivery 尚无真实 valid GLB E2E、幂等/篡改、packaged 设备或用户质量证据，不能作为已交付游戏资产 |
| U004 P2.5 二阶段参考外观烘焙 | 部分实现 | `ReferenceAppearanceBinding@1`、generic sealed-image app-server integration、Core U003/U004 source/raster tests | geometry-first、单次 1024² PBR bake 与 geometry-drift Gate 已实现；仍缺多视图融合、真实 Provider、packaged E2E 与跨类别质量 |
| 游戏资产交付（LOD/碰撞/socket/UV 密度） | 部分实现（本地 runtime 已接线） | `GameAssetProfile@1`、Core `meshopt@0.6.2` deterministic/error-bound LOD tests、`compile_game_asset_lod_delivery` / `verify_game_asset_lod_delivery_glb`、`derive_game_asset_delivery_bindings`、`compile_game_asset_delivery` / `verify_game_asset_delivery_glb`、`GameAssetTexelDensityReadback` LOD0 POSITION/UV0/embedded PNG measurement tests、UAS@2 source→delivery receipt/preview byte lineage、双工件 atomic repository/runtime export、`ForgeAssetPackage@2` source/delivery manifest focused tests | 已有真实 `MSFT_lod`、off-scene AABB collision mesh、socket node、source/profile/UAS Part binding、topology readback 和逐材料实测有效 texel density；runtime 保留 LOD0 用于视觉验收，并用独立 role 保存 delivery GLB，direct GLB/export/package 仅在 receipt 复验后交付 delivery。尚无真实 valid preview→confirm→export/package E2E、幂等/篡改、用户 profile、packaged 设备和质量证据；不达目标密度绝不放行 |
| DeepSeek/千问唯一 AI Provider | 已实现（Alpha） | U004A；`release:ai-provider-policy`、Rust 非法 endpoint/model 反向测试、desktop build、F026 smoke | 真实组合 E2E 与视觉质量仍须单独验证；第三个 Provider 禁止 |
| 同一工作台 PBR 瞬态采集与开放图片比较合同 | 部分实现 | `WorkbenchPbrVisualCapture@1`、固定 `640×640` same-renderer capture、实际 camera world/projection/target/frame pose hash、Core `CandidatePbrCaptureSession/Evidence@1`、Rust app-server one-time registry（PNG IHDR dimensions 重检）、`waiting_for_capture` lifecycle、Tauri 受限 `issue/submit/resume` bridge、一次性工作台授权卡与 Tauri `authorize_candidate_pbr_visual_comparison`、UAS@2 `GenericHardSurfaceAppearanceCompilation@2` 对 1–8 个真实材质区的 sealed normal/roughness/conditional-emissive five-channel bake、ActionLoop 同 Turn evaluate→唯一 typed patch→重新 capture continuation、`VisualEvidenceGraph@2` 到只读 comparison-claim projection（request/profile/feature/plan/V2 graph/program hash 绑定）、`VisualConvergence@2` 单 patch Gate | 未点击授权卡不联网且不产生 preview/version；续跑重算 exact scope，漂移、无 grant 或已失效 grant 全部 fail-closed，patch 清除旧 grant；Appearance Compiler 不等同 UV 投影或照片材质恢复；真实千问调用、packaged 八视图工作台 E2E、未见输入质量与跨类别表示尚未完成；重启会显式终止内存 continuation |
| Deformable、Local Hybrid | 部分实现（仅受限本地硬表面执行分支） | U004 `deformable.local_lattice_shell_v1`：固定 `2×2×2` cage、每轴 `±25%` AABB offset、Python/Rust validator、Worker topology-preserving GLB/readback、VP203 lowering；UAS@2 还可逐部件组合它与 `procedural.generic_hard_surface_v1`，Rust 重验每个 lattice terminal、offset、Part binding 与 sealed RepresentationPlan，对同一 author/build/same-renderer PBR capture/visual convergence/一次 VP204 patch 链路生效；Python 4 tests、Core UAS focused、app-server route/ActionLoop focused | 仅审查过的机械硬表面程序化源可用；没有任意 mesh、角色、生物、布料、mesh seed 或神经混合。真实千问、packaged E2E、confirm/version/Snapshot/export、跨类别净质量与真人门仍缺，不能承诺用户质量 |
| 跨类别正式质量 | blocked | U005 测试策略已冻结 | 八类真实输入、首轮/一次 patch、时间/成本和独立真人盲评均 NOT RUN |
| Packaged 生产发布 | blocked | 本机 macOS Alpha 历史 packaged Gate | sidecar、签名、公证、全新机安装/升级、同 commit 发布证据 |
| Weapon/Concept 兼容 | 兼容基线 | `COMPATIBILITY_MIGRATION.md`、版本化 OpenAPI/migrations | M5/M6 未退出；不得作为新产品能力 |

状态定义：`已实现` 必须有当前代码与 Gate；`部分实现` 必须列出缺口；`目标设计` 没有用户可用承诺；`blocked` 表示退出证据明确缺失。任何目标名称、截图、模型自评或单次 Provider 成功都不能改变状态。
