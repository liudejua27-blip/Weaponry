# ForgeCAD 当前状态账本

版本：2026-07-30
状态：当前能力、目标和阻断的唯一摘要

## 1. 当前一句话结论

ForgeCAD 是具有类别开放理解入口和统一资产源谱系的本机 Alpha：Rust-owned 状态、受限几何、GLB/PBR/readback、唯一结果、确认/版本/导出和重启恢复已有工程证据。U002 已让任意文本、sealed 图片、多视图和活动资产进入同一对象理解/视觉合同/表示计划；U003/U004 已让经过验证的机械臂、`procedural.generic_hard_surface_v1`、受限 `deformable.local_lattice_shell_v1`，以及二者逐部件组合的本地硬表面 Hybrid UAS@2 路径进入同一 author→build→同源 PBR capture→最多一次 VP204 typed patch 工程链。经过同源 capture/evaluate 的非游戏 UAS@2 preview 现在携带 Rust 验证的通用 provenance，可沿既有原子确认事务持久化其 source/hash；它不再伪造旧 Domain Pack、C111 或 V003 decision。Hybrid 只允许审查过的硬表面程序化部件与固定 `2×2×2` cage 外壳部件混合，不是人物、生物、任意网格、mesh seed 或神经重建能力。U004A 已删除 Fal/Hunyuan 远程生成运行时，并把 AI Provider 固定为 DeepSeek 文本设计与千问视觉理解/比较。游戏 delivery 的 source LOD0 与 delivery GLB 现已有专用双工件原子持久化、经 receipt 重验的直接导出，以及 `ForgeAssetPackage@2` 双 hash 包装基础；真实 valid source→delivery preview→confirm→export/package E2E、篡改/幂等、用户 profile、packaged 工作台和跨类别盲测尚未完成，因此仍是 in-progress capability；未具备表示的类别继续返回 typed limitation。

## 2. 当前任务

- `FGC-U001A done`：旧文档已删除，有效兼容事实已并入新权威文档，六项文档 Gate 通过；
- `FGC-U002 done`：Rust-sealed `UniversalAuthorRequest@1`、开放式 `SubjectProfile@1`、`VisualFeatureContract@1`、`RepresentationPlan@1`、typed outcome/limitation、统一入口和模板回退删除；
- `FGC-U003 done`：Rust 派生 `UniversalAssetSource@1`，完成 component/detail/material/camera/projection 合同、编译产物 exact-lineage 与版本 provenance；
- `FGC-U004A done`：DeepSeek/千问唯一 Provider allowlist、旧远程 Mesh 运行时删除、遗留 Fal/OpenAI concept-image director 从 Tauri/产品二进制剥离（仅 test fixture 编译）和防回归 Gate 已通过；
- `FGC-U004 in_progress`：前端 `WorkbenchPbrVisualCapture@1`、Core `CandidatePbrCaptureSession/Evidence@1`、Rust app-server 一次性 registry 和 Tauri 受限 `issue/submit/resume` bridge 已将唯一 renderer 的同源 PBR GLB/hash/环境/固定 render manifest、通用八视图、OS nonce、时限、LRU/TTL 和字节预算绑定为不落盘的临时采集合同；开放 author 现在发布 `CandidatePbrCapturePending@1` 并在 GLB/readback 后停止。`UniversalAssetSource@2` 现由 Rust 编译 `GenericHardSurfaceAppearanceCompilation@2`：它按 sealed profile/feature/material binding 选择最多八个真实 Part/Material Zone，并逐区封存 `SurfaceLayerProgram@1` 与 compilation hash；受限 executor 仅接收这些 lowerings，实际将 normal relief、roughness 与条件式 emissive 送入既有五通道 PBR GLB writer。已注册的 `deformable.local_lattice_shell_v1` 只能将审查过的硬表面程序化源以固定八角点 cage 重塑，UAS 会重算 terminal `lattice_deform`、offset 和 Part binding；它与程序化路径共用 build/readback/capture/evaluate/一次 patch 链，不扩大到任意 mesh、角色、生物或 local-hybrid。它不是 Provider 自报材质、UV rasterization、照片投影或照片级材质恢复。Turn 持久化为 `waiting_for_capture`，工作台自动挂载精确临时 GLB、使用同一 Three.js renderer 采集/提交八视图。带 sealed 图片的开放请求会将已验证的 `VisualEvidenceGraph@2` 投影为只读比较声明，并把 request/profile/feature contract/representation plan/V2 graph/精确程序共同哈希绑定；capture 后工作台显示一次明确的千问比较授权卡，Rust 仅为该 Project/Turn/GLB/投影生成短时预算 grant，点击前不联网且不创建 preview/version，grant 与任一 hash 漂移不匹配即 fail-closed。授权后 capture 被采用的封存 Turn 才恢复 evaluate→preview；若评估失败，ActionLoop 先恢复同一封存 Turn，再只暴露一次 DeepSeek typed patch；patch 生成新 GLB 后必须重新采集和重新授权，第二次失败立即终止，绝不创建 preview/version/Snapshot/Quality/Export 或重新 author。重启期间未保存的 continuation 会明确失败，不会从文本猜测恢复。当前产品 `VisualConvergence@2` 固定一次 author 加最多一次 typed patch；`@1` 仅用于 V003/C111 回归。packaged 八视图 E2E、真实千问比较和真实未见输入仍未运行；之后才扩展 local-hybrid、受限 UV projection、正式固定输出视图比较和版本闭环；
- `U004 P0 evidence hardening`：候选 capture 已固定为同一 Three.js renderer 的 `640×640` physical drawing buffer；Core session 和 accepted view 均记录该尺寸，Rust 从 PNG IHDR 实读并拒绝窗口尺寸漂移。每一视图的 camera proof 现来自实际 world/projection matrix、Orbit target、frame NDC、GLB hash、view slot 与 render manifest，而不是只对视图名称散列。2026-07-30 又将同一 renderer 的 silhouette、normal、depth、part-ID、material-ID 固定为五个 `320×320` GPU pass，并封装为每视图 `960×640` auxiliary contact sheet；Rust 强制其 SHA、IHDR 尺寸、字节预算以及每个 tile 非空且非全图单色的最小 GPU 输出检查与 beauty PNG 同时存在。`desktop:u004-workbench-pbr-capture-playwright` 已用 Rust GLB fixture 在同一真实 browser canvas 采集到完整 bundle；二值 silhouette/单 zone pass 合法通过，depth 有真实色阶。千问仍只接收颜色管理的 PBR beauty，辅助图只作本地 deterministic evidence。它仍是一次性瞬态证据，不等同真实千问评分、packaged GPU E2E 或未见输入质量通过。
- `U004 P1 projection provenance hardening`：`AppearanceProjectionLayer` 现在只有在同一 sealed image 派生的证据 artifact、`UnobservedTexelMask` 和已完整拟合的相机同时存在时才可通过；颜色、法线、粗糙度和金属度通道必须分别匹配合法 artifact，unresolved camera、跨图拼接和 guessed-as-observed 都 fail-closed。当前尚未实现 UV rasterization 或照片像素烘焙，因此这不是照片材质恢复能力。
- `U004 P2 restricted pixel-bake foundation`：Python sidecar 保留 `ReferenceUvEvidenceBake@1` 的 bounded base-color UV rectangle 兼容路径，并新增 `ReferenceCameraUvRasterBake@2`。后者只接受封存 PNG/hash、明确材质区、已验证的 row-major world-to-clip 矩阵和最终 GLB 的受限三角形；Worker 让所有 zone 写入相机空间 depth buffer，仅将可见 target zone 的 source pixels splat 到真实 UV texel，并将 inverse unobserved mask、matrix hash、三角数和覆盖量写入/读回同一 GLB。它拒绝无 retained surface、跨/重复 zone、无效矩阵、无界三角数与伪造 source hash；V1 不被升级包装成真投影。P2.1 已让 Rust 从同一工作台 GPU auxiliary silhouette tile 派生候选 per-mille bounds，并只在 sealed image 的已观察 macro geometry region 达到 IoU/中心误差阈值时，将对应 unresolved `ReferenceCameraHypothesis@1` 替换为置信度上限 8500 的 `silhouette_fit`；不匹配仍保持 unresolved。P2.3 现由 Rust 从 exact candidate GLB SHA、最终 compile-readback bounds 和冻结 turntable slot 可复算地签发 `ProjectionCameraBinding@1`：session 为八视图分别封存 binding hash，app-server 只向同一 Project/Turn 返回已重验的 binding，工作台在唯一 Three.js renderer 上按绑定设置相机并随 beauty/auxiliary capture 提交对应 hash。P2.4 现由 `CoreRepository` 仅按同 Project 的 evidence ID 从不可变 CAS relation 读取 sealed `image/png`，重验 evidence/source SHA、8 MiB 和 worker PNG profile，并结合已重验 `ProjectionCameraBinding@1` 生成瞬态 V2 DTO；前端、Agent 和 Provider 都不能提供路径、URL、source hash、图片 bytes 或矩阵。浏览器实际 camera pose 仍是审计证据，不是 UV 投影真值。没有多视图融合、工作台图片投影入口或完整 ActionLoop pixel-bake wiring；因此仍不是用户可用的图片投影或照片材质恢复。
- `U004 P2.5 ActionLoop 二阶段烘焙`：`ReferenceAppearanceBinding@1` 只能从 observed feature、同 Project sealed PNG、显式 view 与真实 UAS@2 zone 派生。请求封存证据记录 hash，UV DTO 独立重验原始 PNG hash；native executor 先编译 geometry invariant，后才以最多两区 1024² raster bake 进行一次 PBR 二次编译，任何 topology、ShapeProgram、triangle count 或 bounds drift 均 fail-closed。它没有多视图融合、真实千问、packaged E2E 或照片级重建结论。
- `U004 P4 游戏资产交付 runtime`：`compile_game_asset_delivery` / `verify_game_asset_delivery_glb` 将真实 `MSFT_lod`、off-scene AABB collision、socket transform 与 `GameAssetTexelDensityReadback` 一起封存和重算。通用本地 build 现在可先编译并保留同源 LOD0 作 PBR/视觉验收，再由 Rust 派生 delivery GLB；UAS@2 只保存其 exact receipt，preview artifact 同时保留 delivery bytes 和仅供视觉/readback 验收的 source LOD0，避免形成第二资产真值。确认已使用单一 SQLite 事务持久化 `production_glb`/`visual_source_lod0_glb`（source）、`game_delivery_glb`（delivery）和 interactive preview，任何 receipt/hash/role 漂移都 fail-closed。`:model.glb`、`:export` 和 `ForgeAssetPackage@2` 仅在 runtime 复验 source/profile/binding/receipt 后交付 delivery；包清单同时记录 source hash、delivery hash 与 receipt。纹素密度只从最终 LOD0 POSITION/UV0 三角面和实际嵌入 base-color PNG IHDR 计算，`target_met=false` 绝不进入 delivery receipt。尚无用户可选 profile、真实工作台 GLB 的完整 preview→confirm→export/package E2E、篡改/幂等、packaged 设备和用户质量证据，不能将其称为用户可用游戏交付。
- `U004 P3 受限本地形变执行切片`：受限 ShapeProgram runtime 新增 `lattice_deform`：它只允许一个已编译 mesh 输入和固定 `2×2×2` 八角点偏移，每个偏移均限制在源 AABB 各轴 `±25%`，拒绝 profile、空偏移、越界、退化 bounds、退化三角面和静态旋转穿透。Worker 保持原三角拓扑、Part/Zone/material provenance 与 GLB feature history/readback；`ForgeVisualGeometryProgram@2` 可将同一节点 lowering 到该 runtime，不产生第二 DSL。`deformable.local_lattice_shell_v1` 已由 Rust capability registry 选择，`UniversalAssetSource@2` 在 author、build、preview lineage、PBR capture、visual convergence 与一次 VP204 patch 中重验其 source/terminal lattice binding；Core UAS、app-server route 和 ActionLoop route focused tests 通过，Python deterministic/readback 4 项通过。真实 DeepSeek/千问、packaged E2E、confirm/version/export 和跨类别质量未运行，故不称为通用 deformable/local-hybrid 或用户质量承诺。
- `U004 回退清理`：无效 legacy `ForgeVisualProgram` 现在明确返回 schema failure 且保持零候选/零 binding 副作用；C111 helper 已标注为 fixture，不得作为任何类别的 author recovery。冻结 C111/E005 回归保留，但不得由此宣称类别开放质量。
- U005：依赖 U004，当前 `blocked`；
- C111B/E005：仅机械程序化回归；reference comparison、正式 run 和真人质量未完成。

## 3. 已实现的 Alpha 地基

- `FGC-S001`–S008 与 `FGC-Q002`：`ActiveDesignSnapshot@1`、CAS/ETag、选择、preview、quality、undo/redo、导出和恢复保持单一活动真值；
- K001–K003：Rust app-server/core 拥有 Agent、Provider、项目、版本、SQLite/WAL 和 CAS；Python 只执行受限几何；
- G801–G826：受限 ShapeProgram、Profile/Extrude/Revolve/Loft/Sweep、Manifold CSG、UV/tangent/face→Part/Zone 与严格 GLB readback；
- A005/M108A：preview→confirm 的表面程序、五通道 PBR、双档工件和导出；
- F026/V003：一个 renderer、一个结果、失败零永久副作用；
- `FGC-R002`–R004：同源四视图、条件式爆炸图和 PNG/manifest 图包；这些不是工程图或真人质量证据；
- VP201–VP204：typed v2 DAG、宏/repeat、高层几何、一次 author + 最多一次 patch、缓存和恢复；
- PV008：真实 DeepSeek 文本意图已进入 Rust→GLB→preview/confirm/Snapshot/export 机械臂工程链。
- U002：`author_universal_asset` 是新项目首轮唯一工具；八类开放 fixture、机械臂正路径、未知表示零 worker/版本副作用、工作台 limitation/旧资产保留和 R3 回归已有 Gate。
- U003：Provider 不能提交资产源；Rust 从已验证的 request/profile/feature/plan 与当前程序化 revision 派生完整 source，编译后再封存 ShapeProgram、GLB、语义/编译 readback 和固定视图 hash。无求解证据的参考图保持 `unresolved`，不生成伪投影层。

## 4. 能力与阻断账本

| 能力 | 状态 | 主要阻断 |
| --- | --- | --- |
| 当前机械臂/硬表面生成 | 部分实现 | 外观低于目标参考；正式 comparison/真人 4/5 未完成 |
| 类别开放入口 | 已实现（Alpha） | U002 合同、Rust-sealed request、typed limitation 与工作台 Gate 通过；机械臂、受限通用硬表面、local lattice 及其逐部件混合可执行；非游戏 UAS@2 正式 preview 已可沿现有单 GLB 确认事务持久化，其余类别仍 limitation |
| 通用资产源/外观绑定 | 已实现（当前程序化切片） | U003 合同、Rust 派生、编译/预览/版本 exact-lineage Gate 通过；U004 已签发候选 GLB 绑定的固定投影视角，并能由 repository sealed PNG 构造 V2 worker DTO；非游戏 UAS@2 以独立 universal provenance 进入确认，游戏双 GLB 已接入专用原子持久化与受验证导出/包基础；ActionLoop 产品接线、多视图融合和照片 PBR 恢复仍未实现 |
| DeepSeek/千问唯一 AI Provider | 已实现（Alpha） | U004A allowlist 与 `release:ai-provider-policy` 通过；旧远程 Mesh 命令、凭据、adapter、UI 和 live Gate 已删除 |
| Deformable/Local Hybrid | 部分实现（仅硬表面受限分支） | `lattice_deform` 已有受限 Worker、typed schema、GLB/readback 与 VP203 lowering；UAS@2 现能逐部件组合 `procedural.generic_hard_surface_v1` 与 `deformable.local_lattice_shell_v1`，并在 author/build/capture/一次 patch 中重验终端和计划映射。任意 mesh、人物、生物、布料、mesh seed、版本/导出和跨类别质量仍未完成 |
| 跨类别质量 | blocked | U005 八类真实输入、时间/成本和真人评分均 NOT RUN |
| 可编辑资产 | 部分实现 | 自由 split/merge、开放参数化和深度分件未实现 |
| 生产发布 | blocked | sidecar、签名、公证、全新机安装/升级、同 commit 证据不足 |
| Weapon/Concept 兼容 | 兼容基线 | M5/M6 未退出；只按 `COMPATIBILITY_MIGRATION.md` 维护 |

更细状态和 Gate 见 [能力—Gate 矩阵](evidence/CAPABILITY_GATE_MATRIX.md)。

## 5. 文档状态规则

- `已实现`：当前代码路径和对应 Gate 都存在；
- `部分实现`：必须列出已完成与缺失；
- `目标设计`：尚无用户可用实现；
- `兼容基线`：只服务旧库回放/迁移；
- `blocked`：退出证据明确缺失；
- `superseded`：被新任务接续，不等于原目标通过。

用户指南不得把目标设计、旧 evidence、模型自评或单次 Provider 成功写成当前通用能力。

## 6. 文档归属

| 事实 | 权威文档 |
| --- | --- |
| 产品范围 | `PRODUCT_DEFINITION.md`、ADR-0022 |
| 当前用户能力 | `USER_GUIDE.md` |
| 目标架构 | `DESIGN.md` |
| 当前任务 | `CODEX_TASK_INDEX.md` |
| 当前工作区/验证 | `CODEX_HANDOFF.md` |
| 能力/Gate | `evidence/CAPABILITY_GATE_MATRIX.md` |
| 兼容边界 | `COMPATIBILITY_MIGRATION.md` |
| 发布阻断 | `PRODUCTION_RELEASE_CHECKLIST.md` |

## 7. 必读与必跑文档门

按 `DOCUMENTATION_MAP.md → DOCUMENTATION_STATUS.md → CODEX_HANDOFF.md → CODEX_EXECUTION_PLAN.md → CODEX_TASK_INDEX.md` 阅读。文档变更至少运行：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run agent:check
git diff --check
```

任一失败都必须保留为阻断；历史 PASS 不替代当前重跑。
