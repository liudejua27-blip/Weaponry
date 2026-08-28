# ForgeCAD MVP 与生产发布清单

> 2026-08-26 现行 source：**527 schemas / 115 read + 87 write = 202 tools**。真实 D1 网格编辑纵切仍为 owner-evidence-blocked reviewable tradeoff，不勾选任何 commercial release 项；完整 High/Low/UV/Bake、Material evaluator、FPS、Engine 与独立 Hero Art Review 均未通过。

> 商业 FPS 发布新增硬门：canonical/optimized semantic diff、KTX2 decode hash、LOD/collision/socket/animation readback、Unreal clean import/reimport/restart/packaged run、Unity second profile、target-hardware p50/p95/p99、DPT 和 independent Hero Art Review。详见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 商业发布硬门：至少一个同 hash Hero candidate 必须依次通过静态源资产、FPS presentation、LOD/collision/socket、canonical/optimized package semantic diff、clean-project import、packaged target build、frame-time `p50/p95/p99`/memory/streaming、独立 Hero Art Review、user confirm 与 restart/export readback。Three.js、glTF Validator、source compile 或转台 beauty 均不能代替。

2026-08-26 发布阻断补充：Formal High public MCP surface 与 Store scoped idempotency 已 source/focused PASS；positive/restart/cleanup、真实 D1、visual/human/engine/package 仍未通过。因此 515 schemas/194 tools 仍不能勾选 Formal High、Hero Asset 或商业发布项。

版本：2026-08-25
MVP 结论：**功能核心 PASS；参考图真人验收/packaged release BLOCKED 或 NOT_RUN**
MCP010 结论：**FGC-MCP010A done；FGC-MCP010B structural source Gate PASS（Darwin OS memory hard cap deferred/NOT_RUN）；FGC-MCP010C source-focused PASS_WITH_UNRUN_VISUAL_GATES；MCP010D source-focused Operator/Skill + bounded Manifold Boolean Worker/raw structural PASS（current packaged rebuild/视觉子门 NOT_RUN）；MCP010E source-focused AssetPack/UV/PBR/MikkTSpace + packaged structural PASS（xatlas/Validator/视觉子门 NOT_RUN）；MCP010F BLOCKED**
生产发布结论：**BLOCKED，不可外部分发**

商业 Hero Asset 发布结论：**BLOCKED**。AuthoringMesh、Low 与 Hero UV 已有各自窄幅 durable/source receipt；Formal High factory/Store/internal materializer 及 Cage/Bake fixed Worker、七记录 atomic Store/MCP seam 也只有 source/compile/focused PASS。该证据仍不包含完整 cross-version editor、artist Low/UV review、Formal High 完整正向 restart/public surface、当前 D1 正向 Bake receipt或商业视觉验收。FormQuality/secondary-form、正式 High/Low/UV/Cage/Bake、Material Layer Graph、FPS presentation、LOD/collision/socket、commercial engine、独立人审、same-hash export 均未形成完整同候选证据。详细退出门见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。

## 商业 Hero Weapon 11 阶段发布清单（唯一顺序）

同一 `candidate_hash → export_hash` 必须按以下顺序逐项关闭；任一项不是 `PASS`，发布状态继续 `BLOCKED`：

- [ ] 1. `Art Direction/ReferenceViewSet`：`WeaponArtBrief@1`、五核心视图/CameraLock、silhouette/negative-space/landmark、授权与预算；当前 CrossView=`QUALITY_TARGET_NOT_MET`、`secondary-form-approved=NOT_CREATED`、`HQ360=BLOCKED_REFERENCE_COVERAGE`。
- [ ] 2. `AuthoringMesh`：original/evaluated、稳定 V/E/H/C/F/loop/ring/boundary、可编辑历史与 High↔Low correspondence；split/collapse/dissolve **3/3 PASS** 仍仅结构。
- [ ] 3. `High`：非破坏 High/DetailGraph、support/crease/weighted normal/Subdivision、strict GLB readback；source-only，`FPS-HIGH-05=NOT_PASSED`、proposal=`registered=false`。
- [ ] 4. `Low`：artist-authored quad、hard-edge/seam/Part 边界、bake-ready correspondence；`DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`，durable **1/1 PASS** 不等于商业通过。
- [ ] 5. `UV`：2K/4K density、seam/stretch/overlap/OOB/padding、UV0/UV1、tangent/Mikk；Hero UV 7 contracts/public **1/1 PASS**、4 CAS roots linked/GC 仍 structural/source。
- [ ] 6. `Cage/Bake`：对应 Cage、per-Part ray、miss/fallback/cross-part/skew 与 8 类 maps；Worker/public persistence seam source PASS 不可勾选本项，Formal High 完整正向 restart/public surface 与 current-D1 positive receipt 缺失，正式门未通过。
- [ ] 7. `Material`：`MaterialLayerGraph@1` 与 Layer/Mask/Generator/Decal/Wear/Microdetail；当前 **4 MaterialZones / 6 formula textures**，commercial PBR=`NOT_PROVEN`。
- [ ] 8. `LOD`：authored LOD0/1/2、collision/socket、误差与平台预算；commercial LOD/performance=`NOT_RUN`。
- [ ] 9. `Viewer/animation/VFX/audio validation`：同 hash read model、第一/第三人称、动画/VFX/audio、无障碍与可读性；Three.js 仅结构消费，仍 `NOT_RUN`。
- [ ] 10. `Engine`：Unreal 或 Unity importer/material/tangent/LOD/collision/socket/animation round-trip 与预算；**Unreal/Unity 均未运行**。
- [ ] 11. `Independent Hero Art Review`：独立资深艺术家盲审/修订闭合、同 hash confirm/version/export/restart；human=`NOT_RUN`，无 `PASS_HUMAN_ART_REVIEW`。

当前源面 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 MCP tools**、Three.js smoke 和旧 bake 诊断都不能勾选上述发布项。未全部通过前不得生成 `HERO_ASSET_APPROVED`、外部分发包或发布说明。

发布包不得包含或依赖 Blender、Substance、Maya、BlenderMCP、任意 Python/JavaScript/shell 插件、远程 image-to-3D 或运行时联网素材服务。经 accepted adoption 的算法必须作为固定 ForgeCAD module/typed Worker 随 App 分发，并进入 release manifest、SBOM、NOTICE、签名、资源和移除回执。

- [ ] 依赖 scope 复核：Manifold 只允许 bounded same-Part accepted slice；`mikktspace@0.3.0` 只允许 restricted tangent slice。
- [ ] 候选模块排除：OpenSubdiv、QuadriFlow、xatlas、Embree、MaterialX、OIIO、OCIO、meshoptimizer、glTF Validator 均为 `NOT_IN_RELEASE`，除非各自新增 accepted receipt。
- [ ] Validator 复核：外部 glTF Validator 当前 `NOT_RUN`；不得用 Runtime strict readback 或 Three.js smoke勾选外部 Validator/Engine Gate。
- [ ] 每个 accepted module 均具备固定 revision、LICENSE/NOTICE、transitive SBOM、module/binary hash、签名、资源上限与 removal receipt。

## 1. 已完成基座

- [x] MCP001 旧 UI/Provider/App Server/Agent/contracts 成组硬切
- [x] Runtime 是唯一 DB/CAS writer；OS 文件锁、第二实例 `RUNTIME_BUSY`
- [x] 产品代码无内置模型、Provider、模型 API Key、FastAPI/8000
- [x] Runtime V1 不自动打开旧 Library
- [x] MCP003 Codex Desktop/CLI read-only discovery/connection/mismatch scope
- [x] MCP004 candidate/Job/approval/idempotent confirm/reject/restore/diagnostic export
- [x] MCP initialize 不等待 Runtime；缺失/crash 时 stdio survival；一次有界 restart
- [x] 真实 Codex CLI diagnostic write 与 Viewer authenticated read model
- [x] `npm run release:mcp004`

MCP004 历史 manifest 仍保留 signing/reference/Geometry/GLB 的 BLOCKED/NOT_RUN；这些未被视为通过。

## 2. MVP Gate（MCP005–009）

### Reference / Codex

- [x] 真实 PNG/JPEG 原始字节通过 Codex CLI 进入 CAS，source/CAS SHA-256 相同（MCP005）
- [x] MIME/魔数、byte/pixel/dimension/frame/decode memory limits（MCP005）
- [x] symlink/path escape/目录/device/truncation/oversize/hash mismatch fail closed（MCP005）
- [x] DB/log/MCP/evidence 无用户名、绝对路径、原图副本（MCP005）
- [x] Desktop 附件不可验证时明确记录 `NOT_RUN / unavailable`（MCP005）；不伪造 Desktop PASS

### Typed Skills / Geometry

- [x] SubjectProfile/RepresentationPlan/AssemblyGraph/GeometryProgram/AppearanceProgram
- [x] 10 个 first-party declarative Skill 的 Schema/DAG/operator/validator/benchmark/LICENSE/NOTICE/SBOM/provenance
- [x] Bundle canonical hash/trust；unknown operator/cycle/budget/hash/license fail closed
- [x] 机器人真实多 Part mesh/GLB，不是图片平面/单盒/手工成品
- [x] finite/index/normal/degenerate/manifold/budget/source-map strict readback

### Appearance / Viewer / Quality

- [x] bounded UV/tangent/glTF metallic-roughness + typed emissive MaterialZone
- [x] 白外壳/黑机械/橙 emissive typed MaterialZone
- [ ] glTF Validator 0 errors（工具尚未接入；product-owned strict readback PASS）
- [x] Viewer 真实 GLB bytes/candidate hash 通过 authenticated IPC 读取
- [x] Viewer 关闭时 headless beauty/silhouette/normal/part-ID 仍成功（focused worker evidence）
- [ ] reference camera/轮廓/占框/关键比例/区域差异有实际值和 limitation
- [ ] Codex typed review 绑定 pass/region；用户评分有 evidence

### Transaction / Export

- [x] stable Part ID 一次局部修改（bounded recompile；通用 DAG reuse 未宣称）
- [x] reject 无版本；approve 只创建一个不可变子版本
- [x] restore 创建新版本，不改历史
- [x] CAS-backed `mvp-glb` export 绑定 confirmed version/artifact/quality/output hash
- [ ] restart 后真实 Codex Runtime/Viewer/export hash 一致（focused Runtime restart PASS；真实 Codex host restart/Viewer hash 仍 NOT_RUN）
- [x] 真实 Codex CLI 完成当前 MVP 主链：reference→compile→render→evaluate→confirm→version→CAS export（十二调用 receipt）
- [ ] 真实 Codex CLI 完成 reject→change→restore 的完整交互链（当前 focused/runtime evidence 有，真实 host 尚未运行）

全部勾选后只能声明“首个硬表面参考基准 MVP”，不能声明通用高质量或可公开安装。

## 3. 首个硬表面质量 Gate（MCP010A–F）

- [x] 010A：同 revision 用户级开发 App、raw stdio/CLI、用户重启后的真实 Codex capability/project/build hash
- [ ] 010B：source-focused PASS；V2 Schema、真实 GLB/accessor/topology/source readback、损坏输入 fail closed、Worker structural 子门、历史 Dev.app structural probes 已 PASS；Darwin OS 总内存硬门仍 NOT_RUN
- [ ] 010C：perspective/z-buffer 固定 renderer、九 AOV、silhouette/landmark/region metrics、typed review tools
- [x] 010D：12 个真实高细节 Operator、`hard-surface-detail@0.2.0` integrity/benchmark/Worker source Gate；Manifold 固定 revision C API 已通过同一 Part bounded union/difference/intersection adoption/raw Gate；current packaged rebuild、视觉子门未运行
- [x] 010E：first-party 离线 AssetPack、512px UV atlas、固定 `mikktspace@0.3.0`、embedded PNG PBR/texture、逐资产 license/SBOM/provenance 与 source raw Gate；xatlas/Khronos Validator/packaged/视觉子门未运行
- [ ] 010F：Viewer compare/selection/isolate/explosion/a11y、真实 Codex change/confirm/restore/export/restart 同 hash、用户四项评分 ≥4/5
- [ ] 当前单图达到 `PARTIAL_VISIBLE_VIEW_PASS`
- [ ] front/back/left/right/rear-three-quarter 全身视图逐项通过后才记录 `HQ_360_PASS`；缺图时固定 `BLOCKED_REFERENCE_COVERAGE`

完成本节只能声明首个硬表面参考基准，不代表可外部分发或通用高质量。

## 4. 生产发布 Gate（MCP011–013）

### 功能与可靠性

- [ ] Job checkpoint/并发/cancel race/kill-9/disk full/GC/配额/性能
- [ ] 跨类别 Benchmark，展示最差类和失败样本
- [ ] 独立真人盲评

### 供应链与安全

- [ ] 所有 binary/Skill/asset dependency 固定 revision
- [ ] LICENSE/NOTICE、完整 transitive SBOM、provenance、最终分发签名
- [ ] Skill tamper/revocation/upgrade/rollback
- [ ] Worker sandbox、无任意脚本/网络/路径
- [ ] 无 secret/prompt/原图/绝对路径泄露
- [ ] 内容安全和资产授权

### macOS 与宿主

- [ ] Developer ID signing、hardened runtime、最小 entitlements
- [ ] notarization/stapling、`spctl`、clean install
- [ ] 安装器生成无 secret 的 Codex Desktop/CLI 配置
- [ ] signed package 上 Desktop + CLI 完整 write/attachment/Viewer E2E
- [ ] 离线启动、升级失败回滚、DB/CAS backup/restore
- [ ] Runtime/MCP/workers/Viewer/Skills 同合同/manifest/signature

IDE/其他 MCP Client 和 transport-specific official conformance 仍为 future optional，不是个人 MVP 或当前 P0 发布阻断。任何未勾选的生产必需项存在时，不可外部分发。
