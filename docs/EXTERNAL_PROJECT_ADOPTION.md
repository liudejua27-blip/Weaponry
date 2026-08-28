# 外部项目、Blender 与 GitHub 采用清单

> 2026-08-27 foundation materialization 不改变第三方采用状态。Pichuliru、WRAD 与 Lightning 固定哈希母版只作为已审计 importer 输入；进入产品后的权威结果是 Runtime-owned Part-bounded `AuthoringMesh@2` 与其 CAS/SQLite lineage，而不是上游 GLB、Blender 状态或外部仓库。三份母版结构物化/replay/restart 已通过，但复合 FPS package、艺术拓扑、High→Low→UV→Bake、视觉/真人/引擎门仍未证明。

> 2026-08-26 `04AF` 采用结论不变：用户已授权复制、下载与使用合法开源项目，但仅允许经固定 revision、license/SBOM、determinism/resource 审计后进入 isolated typed Worker。Manifold 仅 compile/link seam，OpenSubdiv/Embree/xatlas/QuadriFlow 等尚未因目标计划而自动成为 active Runtime 依赖。真实 D1 失败表明必须先完成自有 AuthoringMesh 的美术编辑能力，外部算法只能加速求值，不能代替 Art Direction。

> 2026-08-26 研究收口：开源库只补算法，不补商业美术真值。QuadriFlow/xatlas 仅 draft，Embree 仅 ray kernel，MaterialX 仅 closed interchange，meshoptimizer/glTF Transform 仅 approved delivery derivatives；TRELLIS.2/Hunyuan3D/SPAR3D 只允许隔离 `DRAFT_UNREVIEWED` concept proposal。完整采用边界见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 更新：Native High/Low/Hero UV 与 Cage/Bake fixed Worker/public persistence seam 都是 ForgeCAD 自有实现，不是外部项目采用；source 进展不改变任何第三方库的 `approved-for-evaluation` 或 `research-authorized` 状态，也不解锁 Blender/Substance/DCC 运行依赖。尤其不能因当前 Worker 已能产生 8-map output，就把 Embree/OIIO/MaterialX/OCIO 写成已采用。

> 2026-08-26 用户授权：允许为本商业武器路线复制、下载、研究并在许可证允许范围内使用开源项目。列入本账本的候选可直接进入隔离 adoption cache 做冻结/许可证/SBOM/benchmark，无需逐仓再次请求；但只有窄范围 `accepted` receipt 才能进入 product Worker/lockfile/package，下载或编译本身不改变产品状态。

版本：2026-08-25
状态：商业游戏武器质量研究已收口为采用队列，但没有新增产品依赖。固定 `mikktspace@0.3.0` 与 MCP010D 固定 revision Manifold C API 仍是仅有的 accepted third-party product slices。xatlas、Khronos Validator、OpenSubdiv、QuadriFlow、Embree、MaterialX、OpenImageIO、OpenColorIO 与 meshoptimizer 仍未进入产品真值。Native High 是 ForgeCAD 自有实现，其 source durable/MCP receipt 不属于第三方采用；proposal 保持 `registered=false`，不构成 active capability 或 High Gate。

## 1. 采用规则

外部仓库只能以四种身份进入：

1. **Library**：链接到受限 Core/Worker；
2. **Tool/Worker**：独立进程、固定输入输出、签名和资源预算；
3. **Asset**：逐资产许可证、hash、作者和来源回执；
4. **Reference only**：只学习算法或交互，不复制代码/资产。

“受控源文件研究”不是第五种产品身份，而是 adoption 前的隔离流程：它只允许经过用户授权、冻结 revision、逐文件 receipt 的上游文件进入非产品研究缓存。它不改变 `Reference only` 的产品边界，也不等于 Library、Tool/Worker 或 Asset 已被采用。

每项必须通过：维护活跃度、许可证/例外、依赖 SBOM、恶意输入、确定性、资源上限、平台打包、性能、替代/退出策略和 Benchmark。禁止整仓复制、自动运行安装脚本、拉取模型权重、执行 arbitrary Python/JavaScript、在 Runtime 内起不受控网络服务或让第三方格式成为第二真值。

产品采用状态只允许：`approved-for-evaluation | accepted | deferred | reference-only | rejected`。研究 receipt 可以额外标为 `research-authorized`，但它不属于产品采用状态。只有作为依赖或二进制采用的 `accepted` 项目才能改 lockfile/安装包。本文件当前只有受限范围的 `mikktspace@0.3.0` 与 Manifold 固定 revision Worker slice 为 accepted product adoption；`ponytail-preflight` 是另行记录的 accepted first-party workflow rewrite，不含上游代码或依赖。Manifold 不作为通用动态库或 Runtime dependency，仅静态编译进隔离 Geometry Worker。

### 1.1 产品内建能力与研究输入的分界

必须由 ForgeCAD 内建并持有真值的能力包括：Form/Reference evidence、AuthoringMesh、Native High、editable Low/Retopo、Hero UV、Cage/Bake、MaterialLayerGraph/AssetPack、FPS review、LOD/Collision/Socket、commercial-engine readback，以及同一 candidate 的 GLB、strict readback、source/Part lineage、hash、CAS、质量门、confirm/version/export/restart。它们必须由 Runtime 唯一写者、closed typed Worker 和可审计 receipt 共同组成；已接受的第三方代码也只能被产品注册为受限实现，不能成为第二真值或直接写库。

可以研究但不能成为 Runtime 真值的内容包括 Blender、Substance Designer/Painter、其他 DCC 工程文件/graph、GitHub 插件与脚本、Three.js/浏览器场景、OpenUSD/MaterialX 交换文档、模型权重与远程 image-to-3D 服务。研究结果只能进入 reference、research receipt、Schema/test 设计或 ForgeCAD 自有 Rust rewrite；不得进入 active Skill、Runtime allowlist、CAS 真值、Stage、candidate/version 或 export。

Native High 的 ForgeCAD-owned `HighMeshArtifact@1` 与 `NativeHighDurable*` 不是第三方采用：bounded typed Worker/GLB sibling 只接受 embedded-only lowering 与 strict readback，durable prepare/get 在 exact AuthoringMesh binding 后双回放并写入 derived CAS/Store link。source Runtime restart/MCP receipt 已通过，但不代表 active Skill、Commercial High Gate 或视觉 PASS；保持 `registered=false` 与 no Stage/confirm/version/export。

## 2. MVP adoption ledger（混合状态，逐行判定）

`accepted` 是精确产品切片；`research-authorized` 只表示已有冻结静态快照；`approved-for-evaluation; snapshot-blocked` 表示允许排队评估但尚无冻结快照。三者不得互换。

| 项目 | 可能用途 | 许可证初筛 | 决策 | 首个任务 / Gate |
|---|---|---|---|---|
| [image-rs/image](https://github.com/image-rs/image) | PNG/JPEG decode/admission | MIT/Apache-2.0 | approved-for-evaluation | MCP005；关闭 default features、只开 PNG/JPEG、decoder limits、恶意图片 |
| [gltf-rs/gltf](https://github.com/gltf-rs/gltf) | Rust GLB strict readback | MIT OR Apache-2.0 | approved-for-evaluation | MCP007；禁外部 URI、buffer/image/size 上限 |
| [Manifold](https://github.com/elalish/manifold) | robust mesh boolean/manifold | Apache-2.0 | **accepted**（固定 revision、隔离 Geometry Worker；同一 Part bounded union/difference/intersection） | MCP010D adoption/product Worker Gate；C API/FFI、面数/时间/内存/拓扑/source IDs/removal；见 `docs/evidence/mcp010d/raw-stdio.json` |
| [xatlas](https://github.com/jpcy/xatlas) | UV unwrap/pack | MIT | research-authorized；not adopted | 已冻结静态快照；当前不安装，产品使用 bounded triangle-chart packer |
| [mikktspace Rust](https://github.com/gltf-rs/mikktspace) | MikkTSpace tangent generation | MIT/Apache-2.0 | **accepted**（仅 MCP010E source-focused Worker） | 固定 0.3.0、源码 revision、crate/license/SBOM receipt、确定性/恶意输入/GLB handedness Gate；见 `docs/evidence/adoption/mikktspace/0.3.0.yaml` |
| [Khronos glTF-Validator](https://github.com/KhronosGroup/glTF-Validator) | GLB 交付验证 | Apache-2.0 | research-authorized；not adopted | 已冻结静态快照；外部报告 `NOT_RUN`，Runtime strict readback 为当前权威 |
| [OpenSubdiv](https://github.com/PixarAnimationStudios/OpenSubdiv) | High 模 subdivision/crease 求值候选 | Tomorrow Open Source Technology License 1.0（非 SPDX，须法务确认） | research-authorized；not adopted | 已冻结静态快照；crease/topology/determinism/resource/redistribution Gate 未闭合 |
| [QuadriFlow](https://github.com/hjwdzh/QuadriFlow) | Low/retopo 候选基线 | README 标 MIT；`LICENSE.txt` 为 BSD-3-Clause 风格并附 enhancement grant，须按固定 revision 法务确认 | approved-for-evaluation；snapshot-blocked | 只生成 draft；必须 `BUILD_FREE_LICENSE=ON` 并排除外部 solver/PATH，完成 Eigen/solver transitive SBOM 后才可评估 |
| [Embree](https://github.com/RenderKit/embree) | High→Low cage bake 射线与诊断候选 | Apache-2.0 | approved-for-evaluation；snapshot-blocked | 尚无冻结快照；offline CPU/resource/determinism/platform Gate 未运行 |
| [MaterialX](https://github.com/AcademySoftwareFoundation/MaterialX) | 材质层/节点语义与交换 | Apache-2.0 | research-authorized；not adopted | 已冻结静态快照；只研究 typed subset，禁止 shader/runtime 成为第二真值 |
| [OpenImageIO](https://github.com/AcademySoftwareFoundation/OpenImageIO) | bake/map 图像 I/O、色彩与通道诊断 | Apache-2.0 | approved-for-evaluation；snapshot-blocked | 尚无冻结快照；codec/恶意图像/内存/色彩/打包 Gate 未运行 |
| [OpenColorIO](https://github.com/AcademySoftwareFoundation/OpenColorIO) | scene-linear/display transform | BSD-3-Clause | research-authorized；not adopted | 已冻结静态快照；config provenance/跨平台确定性 Gate 未闭合 |
| [meshoptimizer](https://github.com/zeux/meshoptimizer) | authored LOD 后的 attribute-aware 优化 | MIT | approved-for-evaluation；snapshot-blocked | 尚无冻结快照；Part/UV/tangent/material/socket/silhouette no-regression 未运行 |
| [glTF-Transform](https://github.com/donmccurdy/glTF-Transform) | GLB inspection/优化 | MIT | approved-for-evaluation-as-dev-tool | MCP009；Node 只在构建/测试，不能写 Runtime 真值 |
| [Basis Universal](https://github.com/BinomialLLC/basis_universal) / [KTX-Software](https://github.com/KhronosGroup/KTX-Software) | KTX2/ETC1S/UASTC 纹理交付 | component-specific；Basis Universal Apache-2.0，KTX 仓含需单独审计的第三方/特殊文件 | benchmark-first；not adopted | color 通常 ETC1S，normal/MR 等 data 优先 UASTC；固定 encoder/thread/profile，保存 source/decoded/compressed 三类 hash |
| [UVAtlas](https://github.com/microsoft/UVAtlas) | UV 算法参考 | MIT；upstream archived/legacy | reference-only | 不作为新产品依赖；仅算法/fixture 参考 |
| [OpenMesh](https://www.graphics.rwth-aachen.de/software/openmesh/) | half-edge 数据结构参考/benchmark | BSD-3-Clause | reference-only / benchmark | ForgeCAD 自有 Rust canonical kernel；不得把 handle/property 当 durable ID |
| [img2threejs](https://github.com/img2threejs/img2threejs) | 分阶段 image → typed spec → procedural review 的工作流思想 | Apache-2.0 | approved-for-evaluation / first-party reimplementation | MCP006；仅学习 staged passes、detail inventory、per-region confidence 和 side-by-side review；不安装其 Python/TypeScript skill，不把 Three.js/JS 作为 Runtime 真值 |
| Blender / Blender headless | Modifier/Depsgraph/PBR/UV/Bake/AOV 的 reference/security/license 研究 | Blender `GPL-2.0-or-later` | **reference-only / unavailable-for-product**（ADR-0028 已降级为非产品研究） | 不执行、不打包、不改 lockfile/Runtime allowlist；只将公开方法重写为 ForgeCAD 自有 Schema/Rust Worker |
| Substance Designer/Painter | 材质层、通道语义、烘焙和贴图 authoring workflow 参考 | 商业软件/产品资产许可另审 | **reference-only / unavailable-for-product** | 不执行、不打包、不接入 SDK/插件/工程 graph；只把方法重写为 ForgeCAD `MaterialLayerGraph`/AssetPack/PBR typed contracts |

“许可证初筛”不是法律批准。当前 `mikktspace@0.3.0` 与 Manifold 固定 revision 已通过各自受限 Worker/source receipt；UV atlas 仍是 ForgeCAD 自有的 512px bounded chart packer，xatlas/Validator 尚未安装。distribution legal review、最终二进制 SBOM 和签名仍在 MCP012/013。

Blender 的产品身份固定为 `reference-only / unavailable-for-product`。没有 Blender binary、Python bundle、`.blend`、lockfile、package、active Skill 或 Runtime allowlist 进入产品；相关 capability projection 必须保持 `unavailable`。ADR-0028 只保留历史威胁模型与许可证研究，不再是未来产品 Tool/Worker 的晋级入口。

### 2.1 img2threejs 研究快照（2026-08-12）

本次阅读了 upstream repository 的 README 与 `SKILL.md`。最值得移植的是方法，而不是运行时：先做 detail inventory 和质量合同，再按 `blockout → structural → form → material → lighting → interaction → optimization` 分阶段生成；每个可见特征必须落到有名字的组件/材质条目；每轮都用受控相机把 render 与 reference 对照，并对关键区域单独记录 confidence；单张图无法证明的背面和隐藏结构必须标为 approximate/unknown，而不是伪造确定性。upstream 明确把结果定位为 code-only、procedural、可编辑的 Three.js 场景，而不是不可编辑的黑盒 mesh。[img2threejs repository](https://github.com/img2threejs/img2threejs)、[upstream SKILL.md](https://github.com/img2threejs/img2threejs/blob/main/SKILL.md)

ForgeCAD 已把这些原则映射到自己的边界：`GeometryProgram@2`/semantic Part/Operator Catalog 对应可编辑组件；`reference_compare_prepare`、九 AOV、`visual_review_submit` 和 `quality_get` 对应分阶段回看；Skill recipe 与 evidence manifest 对应 detail inventory/quality contract；unknown coverage 与 `BLOCKED_REFERENCE_COVERAGE` 对应单图不可见区域。当前没有安装 upstream Python/TypeScript skill，也没有把 Three.js、浏览器预览、Hosted Converter 或其任意脚本变成 Runtime 真值；产品真值仍是 Rust Worker 的 typed program、GLB BIN/accessor 回读、CAS hash 和用户确认。

这次研究还明确了下一项产品缺口：ForgeCAD 已能测整体 silhouette IoU，但 boundary F1、landmark、region detail 的修正仍主要依赖 Codex 判断；本轮新增的 `scripts/make_mcp010f_comparison_sheet.py` 将 reference/beauty/silhouette/diagnostic AOV 固定打包为一张标准库 review sheet，让 Codex 只做视觉判断，manifest 只保存 hash，不替代 Runtime 质量真值。下一步仍应增强“按区域的可见特征清单 + 局部 comparison 修正”，而不是引入远程 image-to-3D API 或插件市场。

### 2.2 其他上游研究快照（2026-08-12）

- [`pmndrs/gltfjsx`](https://github.com/pmndrs/gltfjsx)：学习其“命名 node/material 图、可复用实例、清理冗余 transform”的消费侧思想；ForgeCAD 只把它映射为稳定 Part/MaterialZone 名称、Viewer read model 和导出前性能检查，不运行 gltfjsx、不让 React/JSX 成为 Runtime 真值。
- [`mrdoob/three.js`](https://github.com/mrdoob/three.js)：学习 `PerspectiveCamera`、物理材质色彩空间和 AOV/后处理的 Viewer 表达方式；产品固定 renderer/GLB readback 仍由 Rust Worker 真值负责，Three.js 只读展示。
- [`jpcy/xatlas`](https://github.com/jpcy/xatlas)：学习 chart segmentation、atlas packing 和 seams/texel density 的可验证输出；当前仍 `approved-for-evaluation`，没有把未验证的第三方 unwrap 写入产品包。
- [`microsoft/TRELLIS`](https://github.com/microsoft/TRELLIS)：确认 image-conditioned mesh/GLB 是提升单图前脸细节的潜在路线，但其权重、CUDA/conda 依赖和 GPU 运行时与 ForgeCAD 离线、无内置模型的 MVP 边界冲突，因此只保留为未来明确 opt-in 的 external-base-mesh 研究，不下载权重、不接入 Runtime。

### 2.3 Luna GitHub 受控复刻授权（2026-08-13）

用户已授权 Luna 围绕 build123d、BlenderMCP、CadQuery、Manifold、MaterialX 做重点研究、学习和选择性源文件复刻。授权的实际操作、冻结 commit、候选文件、许可证文件 Git blob、禁止能力和后续 Gate 由 `docs/LUNA_GITHUB_REPLICATION_PLAYBOOK.md` 与五份 `docs/evidence/adoption/<project>/<revision>.yaml` 共同定义。

| 项目 | 当前研究处置 | 允许产物 | 明确不允许 |
|---|---|---|---|
| build123d | `research-authorized` | Parametric Design Kit 的自有 schema/Rust rewrite | Python/OCCT/VTK/Jupyter 进入 Runtime |
| CadQuery | `research-authorized` | bounded selector/Sketch/assembly intent 的自有设计 | CadQuery/OCP script、GUI 或 FreeCAD binding |
| BlenderMCP | `research-authorized`，仅安全/协议研究 | read-only observe、render evidence、tool receipt 的自有合同 | Blender Python、`exec()`、socket、遥测、网络资产 API |
| Manifold | `accepted`（product-owned isolated Worker；`boolean@1` bounded） | C API/FFI、MeshGL64 readback、union/difference/intersection、资源/确定性/移除 benchmark | 自动上游构建、任意 mesh、Python/JS/WASM binding、写 Runtime state |
| MaterialX | `research-authorized` | MaterialZone/PBR graph translator 的自有 schema | shader/render/Viewer/JS/Python runtime 引入 |

研究副本默认只允许存放于受控 adoption cache 或 quarantine；Manifold 是本轮唯一已完成产品 Worker adoption 的例外，`vendored_files` 与 hash 由其 receipt 固定。其余研究项目仍不能提交为 active 模块。

### 2.4 Ponytail 前置工作流重写（2026-08-13）

[`DietrichGebert/ponytail`](https://github.com/DietrichGebert/ponytail) 在固定
`2ed6c52c9d7e5e56942508591085fd45dea277d3` revision、MIT 许可证下被接受为
**workflow reference only**。ForgeCAD 只重写了“必要性 → 复用既有能力 → 最小
typed action”的决策顺序，落地为 first-party `ponytail-preflight@0.1.0` 和 MCP
session-order policy；详见其 receipt 与 `CODEX_PONYTAIL_PREFLIGHT_WORKFLOW.md`。

没有复制或运行上游 Source，没有修改 Cargo/npm lockfile，没有安装其 Node package、
lifecycle hook 或 MCP server。该 accepted receipt 不是可分发 third-party dependency，
也不把引用 Skill、bundle integrity 或会话顺序升级为几何/视觉质量通过。

## 2.5 商业算法的 ForgeCAD-owned 封装边界（future / queued）

Manifold、OpenSubdiv、QuadriFlow、xatlas、Embree、MaterialX、OpenImageIO（OIIO）、OpenColorIO（OCIO）、meshoptimizer 与 Khronos glTF Validator 都只能走同一条采用路径：固定 revision → 逐文件许可证/NOTICE 审计 → transitive SPDX SBOM → 恶意输入/资源限制 → deterministic benchmark → removal plan → ForgeCAD-owned typed Worker → package/signature/hash receipt。GitHub 仓库、动态库、插件、联网服务或上游脚本均不能直接成为 Runtime/Skill 依赖。

封装后的模块统一声明 `ForgeCadModule@1`，至少包括 `schema_refs`、`operator_refs`、`budget`、`fixture_refs`、`license_text_sha256`、`notice_sha256`、`sbom_sha256`、`provenance`（source revision/toolchain/build cohort）、`signature`、`module_sha256`、`contract_set_sha256` 与 input/output hashes；capability 必须显式为 `network=false`、`dynamic_plugin=false`、`script=false`、`direct_db_write=false`、`direct_cas_write=false`。正向、负向、损坏、超预算、重放和跨平台 fixture 不得含用户图片、prompt、secret 或绝对路径。缺一项 receipt 就保持 `approved-for-evaluation`/`research-authorized`/`queued`，不进入 active package、lockfile、Runtime allowlist 或 SBOM accepted 集合。

| 候选 | 目标内建模块 | 当前采用状态 |
|---|---|---|
| Manifold | bounded `NativeHighBooleanWorker` / `boolean@1` | 仅同一 Part bounded accepted Worker；不是通用 mesh、插件或第二 writer |
| OpenSubdiv | `NativeHighSubdivisionWorker` | `research-authorized`；`NOT_RUN`，许可证/确定性/资源/打包 Gate 未闭合 |
| QuadriFlow | `RetopologyDraftWorker` | `snapshot-blocked`；README 的 MIT 标签与实际 `LICENSE.txt` 文本不一致，且 Eigen Sparse Cholesky 路径含 LGPL 风险；必须固定 revision、使用 `BUILD_FREE_LICENSE=ON`、完成 transitive SBOM/法务后才可评估，输出仍只允许 draft |
| xatlas | `HeroUvDraftWorker` | `research-authorized`/未安装；当前 Hero UV durable 是 ForgeCAD source structural slice |
| Embree | `CageBakeRayWorker` | `approved-for-evaluation`；miss/skew/cross-part、平台和资源 Gate `NOT_RUN` |
| MaterialX | `MaterialLayerTranslator` | `research-authorized`；只定义 ForgeCAD typed subset，不执行 shader/runtime |
| OIIO / OCIO | `SurfaceTextureWorker` / `ColorPolicyWorker` | `approved-for-evaluation`；codec/config/色彩/内存 Gate `NOT_RUN` |
| meshoptimizer | `LodOptimizationWorker` | `approved-for-evaluation`；只消费已批准 Low/LOD，属性/轮廓 no-regression `NOT_RUN` |
| glTF Validator | `EngineDeliveryValidator` | `research-authorized`；外部报告不能替代 Runtime readback 或 `EngineValidationReceipt@1` |
| glTF Transform | `AssetPackagingWorker` | `approved-for-evaluation`；只开放 prune/dedup/meshopt/KTX2 等固定操作，禁止任意 JS/CLI 参数或覆盖 canonical source GLB |
| Basis Universal / KTX2 | `TextureCompressionWorker` | `benchmark-first`；按 normal/data/color slot 固定 profile，要求 decoded pixel hash/quality、平台支持和 removal receipt |

Blender、Substance Designer/Painter、Maya 和任意 DCC 仍为 `reference-only / unavailable-for-product`：不复制、不执行、不联网、不打包、不进入 lockfile、Skill registry、Runtime allowlist 或 CAS 真值。

## 3. Deferred / benchmark-first

| 项目 | 用途 | 决策理由 |
|---|---|---|
| [truck](https://github.com/ricosjp/truck) | Rust B-rep/NURBS CAD kernel | benchmark-first；能力和依赖面超过首个 mesh vertical slice |
| [Parry](https://github.com/dimforge/parry) | collision/query | deferred；MCP010F 可先用 product-owned bbox explosion，采用 Parry 仍需独立 receipt |
| OpenEXR/OpenCV/Filament | 高级图像、第二 renderer | deferred；不是首条 Hero Weapon 纵向链的前置条件 |
| [OpenUSD](https://github.com/PixarAnimationStudios/OpenUSD) | scene graph、layer、variant、reference、交换 | reference-only / post-MVP benchmark；只学习 SemanticSceneGraph/ReferenceCanvas 思想，不进入 V1 真值 |
| [Trimesh](https://github.com/mikedh/trimesh) | mesh analysis/repair/test reference | deferred / dev-reference；Python 库不进入 Runtime 真值，未来只能做离线验证或隔离工具 |
| [NVIDIA Omniverse Kit](https://docs.omniverse.nvidia.com/dev-guide/latest/kit-architecture.html) | extension/app composition、headless/UI split、USD-based workbench architecture | reference-only；不引入 Kit SDK、Carbonite、USD runtime 或插件系统 |
| Pi Agent | minimal linear agent harness、skills/extensions/prompt templates | reference-only；只学习 harness 形态，不把 Pi runtime 变成产品状态或 P0 dependency |

## 4. Reference-only / rejected 项目

| 项目 | 学习点 | 不直接采用原因 |
|---|---|---|
| [Blender](https://github.com/blender/blender) | Data-block、Modifier/Geometry Nodes、Principled、UV/Bake、AOV、OCIO、Asset Browser、Outliner | 官方 source/reference 与 headless 路径均为 `reference-only / unavailable-for-product`；ADR-0028 只保留非产品研究；GPL、任意 Python、`.blend` 不能进入产品真值 |
| Substance Designer/Painter | material graph、channel packing、bake/export UX | 仅学习参考；工程文件、graph、插件、SDK、脚本和会话状态不能进入 Runtime/active Skill/CAS 真值 |
| [BlenderMCP](https://github.com/ahujasid/blender-mcp) | DCC tool choreography | rejected for MVP；可执行任意 Blender Python、socket/网络资产，已公开报告 unrestricted `exec()` 风险 |
| [img2css](https://github.com/javierbyte/img2css) | 像素采样、颜色/轮廓预览和轻量 reference visualizer 思想 | BSD-3-Clause；CSS box-shadow/base64 输出只用于离线预览，不能进入 GeometryProgram、不能执行任意 JS/HTML |
| [FreeCAD](https://github.com/FreeCAD/FreeCAD) / FreeCAD MCP | Document、transaction、parametric recompute、workbench/undo | reference-only；不接入 FreeCAD MCP，不把工程 CAD 状态、任意 Python 或文件系统暴露给 Runtime |
| [build123d](https://github.com/gumyr/build123d) / [CadQuery](https://github.com/CadQuery/cadquery) | AI 友好的参数化 CAD API、OCCT/BREP modeling style | reference-only；当前只映射为 Parametric Design Kit typed JSON macro，不执行任意 Python/CadQuery script |
| TripoSR / [TRELLIS.2](https://github.com/microsoft/TRELLIS.2) / [Hunyuan3D](https://github.com/tencent-hunyuan/hunyuan3d-2.1) / 其他 image-to-3D | draft mesh、候选导入合同、PBR draft research | rejected for MVP / future opt-in research；不内置权重/GPU/远程 3D Provider，不能直接 confirm/export，未来另立 ADR |

普通 `Reference only` 必须保存研究链接和自研设计理由，但不复制源文件、提示词包或素材。第 2.3 节五个指定项目是唯一例外：Luna 可以依照受控复刻手册把精确文件放入研究缓存；它们仍不得进入 active 模块、Skill、Runtime、lockfile 或安装包。

## 4.1 FPS 武器生产骨架资产（2026-08-27）

用户授权后，本轮已从官方来源冻结并下载四组 CC0 资产，形成
`packages/forgecad-assets/forgecad-fps-production-foundation/0.1.0-proposal`：Kenney
Blaster Kit 2.1 用于轮廓族参考；Quaternius Sci-Fi Modular Gun Pack 用于模块化
Body/Barrel/Grip/Magazine/Stock 词汇；Pichuliru Flat Guns/Attachments 提供带
Magazine/Trigger/Bolt/Stock 与附件插槽的武器 skin 骨架；WRAD ARMS 提供左右手、
wrist IK、arm target 与 grip socket 的第一人称手臂骨架。所有归档/文件 hash、来源、
许可证、SBOM、用途和移除计划均记录在对应 `docs/evidence/adoption/**` receipt。

该包当前是 `evaluation-only / runtime_active=false`。允许用于离线观察、语义映射、
socket 规范化和 ForgeCAD 自有虚构武器母版的派生；另冻结了 CC0 Lightning Pump
Action Rifle 作为 High→Low→PBR Bake 与基础 skin/animation 的 readback benchmark，
其 High/Low `.blend` 只留在隔离缓存。禁止把上游低模、尺寸、`.blend`
或 armature 直接写成 Runtime/CAS 真值。外部 benchmark 的 High/Low/bake 存在不等于
ForgeCAD 已拥有可编辑闭环；完整 Hero PBR、FPS 动画、引擎回读和独立 Hero Art Review
仍未完成，故不能宣称商业级母版已完成。

2026-08-27 已完成 typed importer 的首个 source/runtime slice：只允许
`pichuliru-weapon-west`、`wrad-arms`、`lightning-low-pbr` 三个固定 hash 的内嵌 GLB，
执行 strict GLB2 readback、累计 node TRS、米制右手坐标归一化、面积 `<1e-12 m²`
退化面清理、稳定 Part/face/vertex 规则、socket/rig/animation/PBR inventory，并把紧凑
ForgeCAD topology、socket map、rig map 和 `FpsPresentationPackage@1` 草案写入 CAS。
Runtime/Store prepare 已通过首次写入、同请求幂等 replay 与 get hash revalidation；MCP
只返回 hash/status，不接受路径、URL 或 source bytes。该 slice 仍为
`structural_only + DRAFT_UNREVIEWED + AUTHORING_MESH_MATERIALIZATION_PENDING`；WRAD/Lightning
方向映射仍是 `PENDING_SOURCE_VERIFICATION`，其余 pack 资产仍为 reference-only。
正式晋级仍需按 Part 物化 ForgeCAD-owned AuthoringMesh、补齐 clips/events、High→Low→UV→Bake
lineage、第一人称视图门、引擎回读和独立 Hero Art Review。

## 5. 材质与 HDRI 资产候选

- [Poly Haven](https://polyhaven.com/)：逐资产 CC0 回执；API/站点代码许可证与资产许可证分开处理；
- [ambientCG](https://ambientcg.com/)：逐资产 CC0 回执；保留原始 ID、hash、分辨率、通道、单位和下载时间；
- 其他 marketplace/社区包：默认不自动导入，先确认每个资产及再分发条款。

即使是 CC0，也必须记录 source URL/ID、retrieved_at、SHA-256、作者、许可证文本 hash、物理尺寸、色彩空间和通道语义。远程链接不能成为版本真值，入库后只引用 CAS hash。

MCP005–009 首个机器人仍只使用 typed procedural/PBR values。MCP010E 计划由 Codex 一次性下载指定的 ambientCG `Metal010` 2K PNG、`Plastic006` 2K PNG 和 Poly Haven `Studio Small 03` HDRI 到本机 adoption cache，不调用 API。每项先固定下载文件 hash、CC0 license text hash、作者/source ID、通道/色彩空间、派生 Recipe 和 SBOM；原 ZIP 不进入 Git。只有逐资产 receipt 通过后，派生内容才能进入 first-party `forgecad-hard-surface-robot@1.0.0` 离线 AssetPack。

Runtime、Viewer 和安装器不得联网或接收素材 URL；远程链接不进入版本真值。010E 不实现通用 pack 安装/升级/撤销，这些属于 MCP012。

## 6. Blender worker 历史研究边界（产品路径已拒绝）

ADR-0028 现只保留非产品威胁模型与许可证研究，不再授权产品隔离评估 lane。Blender 官方 source/binary 仍不复制、不链接、不执行、不打包；相关能力固定 `unavailable-for-product`。下列条目只是若未来重新立 ADR 时必须重新满足的历史风险清单，不是当前实施步骤或晋级条件：

- 历史候选曾要求仅由产品发布并签名的固定 Recipe 启动 headless worker；当前产品禁止启动该 worker；
- 输入是受限 Scene/Material/Bake/Render 合同，输出先由 Runtime hash/readback，再由 Runtime 拥有 CAS 工件和 receipt；
- 禁止 Codex/Skill/用户传 Python、addon、`.blend` 宏、任意文件路径或网络 URL；
- 独立临时目录、无网络、最小文件授权、CPU/GPU/内存/时间限制；
- `.blend` 只是中间缓存，不是项目真值；
- 当前产品直接标记 Blender capability 为 unavailable，不存在 Blender fallback；
- Runtime 仍是 SQLite/CAS/candidate/version/Stage/rollback 唯一写者，Worker 不可直接写入；
- 固定 binary、Recipe、冻结 Python bundle、license/SPDX/SBOM、签名/provenance、negative/security、determinism、resource、restart/rollback Gate 未完成前，不进入 lockfile/package/installer；
- 分发前完成 GPL、动态/进程边界、源码提供义务和 NOTICE 的法律审查。

当前不再创建 Blender render/evidence 评估 receipt。High/Low/UV/Cage/Bake 只由 ForgeCAD 原生 typed Worker 生产，并在独立 contract、strict readback、同 candidate/hash/lineage、Stage@3 quality 和 human/engine Gate 下验收；任何外部 DCC 输出都不能推进 Stage、confirm、version 或 export。

## 7. 采用记录模板

每项采用必须新增 receipt：

```yaml
project: xatlas
source_url: https://github.com/jpcy/xatlas
revision: <full commit>
identity: library
license_spdx: MIT
license_files_sha256: []
transitive_sbom_sha256: <sha256>
vendored_files: []
patches: []
capabilities: [uv_unwrap, uv_pack]
denied_capabilities: [network, filesystem_write, dynamic_code]
benchmarks: []
security_tests: []
platforms: []
fallback_or_removal_plan: <text>
approval: pending
```

没有 `approval: accepted` 和通过的 Benchmark，不得写入 production Cargo/package/installer。

Receipt 放在 `docs/evidence/adoption/<project>/<revision>.yaml`，不得在文件中记录本机 clone 绝对路径。若评估失败，保留 `rejected` 原因和删除/回滚结果，不能只删记录。
