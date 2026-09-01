# 外部项目、Blender 与 GitHub 采用清单

> **ADR-0030 knife-first exception:** Blender 与具体插件可从 `reference-only` 进入
> `isolated-prototype`，但每个候选必须固定 revision、许可证、依赖锁、headless 行为、资源预算、
> 刀类 fixture、确定性差分测试和 removal plan。该状态不等于 adopted、可分发或 Rust-native；
> caller-supplied Python/add-on、Runtime 联网和 `.blend` 真值仍禁止。

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

采用优先级只按穿越火线武器生产缺口排序：robust Boolean/mesh repair、Subdivision、UV/pack、
Cage/Bake/ray diagnostics、tangent/normal、texture compression、glTF/FBX/engine validation。
Blender/BlenderMCP 继续 reference-only；img2threejs 仅在 ADR-0031 独立 Three.js 路线作为
固定源码兼容基线，不是 Runtime dependency；BlenderTools 只借鉴交付纪律。任何库进入
Runtime 前仍需冻结 revision、许可证/SBOM、determinism/resource/adversarial Benchmark 和退出方案。

> 2026-08-27 foundation materialization 不改变第三方采用状态。Pichuliru、WRAD 与 Lightning 固定哈希母版只作为已审计 importer 输入；进入产品后的权威结果是 Runtime-owned Part-bounded `AuthoringMesh@2` 与其 CAS/SQLite lineage，而不是上游 GLB、Blender 状态或外部仓库。三份母版结构物化/replay/restart 已通过，但复合 FPS package、艺术拓扑、High→Low→UV→Bake、视觉/真人/引擎门仍未证明。

> 2026-08-26 `04AF` 采用结论不变：用户已授权复制、下载与使用合法开源项目，但仅允许经固定 revision、license/SBOM、determinism/resource 审计后进入 isolated typed Worker。Manifold 仅 compile/link seam，OpenSubdiv/Embree/xatlas/QuadriFlow 等尚未因目标计划而自动成为 active Runtime 依赖。真实 D1 失败表明必须先完成自有 AuthoringMesh 的美术编辑能力，外部算法只能加速求值，不能代替 Art Direction。

> 2026-08-26 研究收口：开源库只补算法，不补商业美术真值。QuadriFlow/xatlas 仅 draft，Embree 仅 ray kernel，MaterialX 仅 closed interchange，meshoptimizer/glTF Transform 仅 approved delivery derivatives；TRELLIS.2/Hunyuan3D/SPAR3D 只允许隔离 `DRAFT_UNREVIEWED` concept proposal。完整采用边界见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 更新：Native High/Low/Hero UV 与 Cage/Bake fixed Worker/public persistence seam 都是 ForgeCAD 自有实现，不是外部项目采用；source 进展不改变任何第三方库的 `approved-for-evaluation` 或 `research-authorized` 状态，也不解锁 Blender/Substance/DCC 运行依赖。尤其不能因当前 Worker 已能产生 8-map output，就把 Embree/OIIO/MaterialX/OCIO 写成已采用。

> 2026-08-26 用户授权：允许为本商业武器路线复制、下载、研究并在许可证允许范围内使用开源项目。列入本账本的候选可直接进入隔离 adoption cache 做冻结/许可证/SBOM/benchmark，无需逐仓再次请求；但只有窄范围 `accepted` receipt 才能进入 product Worker/lockfile/package，下载或编译本身不改变产品状态。

版本：2026-08-31
状态：ADR-0031 已接受 pinned `img2threejs@9fbd0ca5bbcc3b13bebe712745d6784d33db0b85`
作为独立 Three.js Knife Studio 路线的 Apache-2.0 上游基线；LICENSE/NOTICE/SBOM/provenance、
隔离源码恢复与 bounded 静态 adapter 已通过。pinned validator/generator 现仅在一次性临时目录的
离线 benchmark 中按固定 fixture 执行，7 meshes/1,049 tris 与重复 receipt hash 已验证，strict quality
仍为 `BYPASSED_FOR_FIXTURE`。上游 executable/runtime dependency 仍未采用，不会进入当前 binary；
Three.js 包只接收封闭的内存数据。Rust 商业 DCC 路线的 accepted
third-party product slices 仍只有固定 `mikktspace@0.3.0` 与 Manifold Worker slice。

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
| [img2threejs](https://github.com/img2threejs/img2threejs) | ObjectSculptSpec、程序化 Three.js factory、分阶段生成、浏览器比较与 CS2 刀类基线 | Apache-2.0 | **accepted source baseline / isolated generator benchmark**（仅 ADR-0031 独立 Three.js 路线；不是 Runtime dependency） | `WPN-THREE-ADOPT-001`；commit、LICENSE/NOTICE/SBOM/provenance、隔离恢复与 bounded import 已闭合；pinned validator/generator 仅在临时目录离线跑固定 fixture，禁止网络/安装/Runtime 写入；generated THREE.Group/GLB 仍是可重建派生资产 |
| Blender / Blender headless | Modifier/Depsgraph/PBR/UV/Bake/AOV 的 reference/security/license 研究 | Blender `GPL-2.0-or-later` | **reference-only / unavailable-for-product**（ADR-0028 已降级为非产品研究） | 不执行、不打包、不改 lockfile/Runtime allowlist；只将公开方法重写为 ForgeCAD 自有 Schema/Rust Worker |
| Substance Designer/Painter | 材质层、通道语义、烘焙和贴图 authoring workflow 参考 | 商业软件/产品资产许可另审 | **reference-only / unavailable-for-product** | 不执行、不打包、不接入 SDK/插件/工程 graph；只把方法重写为 ForgeCAD `MaterialLayerGraph`/AssetPack/PBR typed contracts |

“许可证初筛”不是法律批准。当前 `mikktspace@0.3.0` 与 Manifold 固定 revision 已通过各自受限 Worker/source receipt；UV atlas 仍是 ForgeCAD 自有的 512px bounded chart packer，xatlas/Validator 尚未安装。distribution legal review、最终二进制 SBOM 和签名仍在 MCP012/013。

Blender 的产品身份固定为 `reference-only / unavailable-for-product`。没有 Blender binary、Python bundle、`.blend`、lockfile、package、active Skill 或 Runtime allowlist 进入产品；相关 capability projection 必须保持 `unavailable`。ADR-0028 只保留历史威胁模型与许可证研究，不再是未来产品 Tool/Worker 的晋级入口。

### 2.1 img2threejs 研究快照（2026-08-12）

2026-08-31 decision successor：ADR-0031 改变的是独立 Three.js Knife Studio 路线，
不是 Rust 商业 DCC 路线。新路线完整接受 pinned upstream 的 ObjectSculptSpec、stage machinery、
factory generator、browser renderer 和 CS2 knife route 作为兼容基线；随后统一归一化为
Weaponry-owned `KnifeSceneProgram@1` 和 `KnifeObjectiveLedger@1`。当前已固定并验证可恢复的上游
源码快照，完成 license/SBOM/provenance 和 bounded 静态 adapter。2026-09-01 起只允许 benchmark
runner 从本机 git object 恢复 pinned commit，在临时目录离线执行固定 validator/generator，并用仓库
既有 Three.js 依赖验证派生工厂；该运行已得到 7 meshes/1,049 tris、确定性 receipt，但 strict quality
明确绕过。它没有升级为运行时依赖；后续只有独立 worker 的输入/输出、资源和确定性 Gate 闭合后，
才可讨论执行型采用，源码基线或一次 benchmark 本身不授予该状态。

2026-08-30 successor：已对 upstream `main@9fbd0ca5bbcc3b13bebe712745d6784d33db0b85`
进行新的只读静态审计，冻结文件 hash 与 Apache-2.0 receipt 至
`docs/evidence/adoption/img2threejs/9fbd0ca5bbcc3b13bebe712745d6784d33db0b85.yaml`。
当前 Weaponry 采用设计见 `WEAPONRY_KNIFE_REFERENCE_CONVERGENCE_DESIGN_20260830.md`：
只重写 evidence-first intake、detail binding、pass state、deterministic-first review 和 bounded
correction；不安装/执行上游 Python/TypeScript/Three.js，也不直接采用其待校准 CS2 阈值。

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

## 6. Blender public worker 历史研究边界（由 ADR-0030 局部替代）

ADR-0028 现只保留旧的公共 worker 威胁模型与许可证研究，不授权恢复通用 Blender MCP、
任意 Python 或 `.blend` 真值。ADR-0030 只为刀类固定 provider 打开新的
`isolated-prototype` lane，具体执行边界以 6.1 为准。下列条目是旧公共 worker 的历史风险
清单，不覆盖 6.1，也不是产品能力 PASS：

- 历史候选曾要求仅由产品发布并签名的固定 Recipe 启动 headless worker；当前产品禁止启动该 worker；
- 输入是受限 Scene/Material/Bake/Render 合同，输出先由 Runtime hash/readback，再由 Runtime 拥有 CAS 工件和 receipt；
- 禁止 Codex/Skill/用户传 Python、addon、`.blend` 宏、任意文件路径或网络 URL；
- 独立临时目录、无网络、最小文件授权、CPU/GPU/内存/时间限制；
- `.blend` 只是中间缓存，不是项目真值；
- 当前产品直接标记 Blender capability 为 unavailable，不存在 Blender fallback；
- Runtime 仍是 SQLite/CAS/candidate/version/Stage/rollback 唯一写者，Worker 不可直接写入；
- 固定 binary、Recipe、冻结 Python bundle、license/SPDX/SBOM、签名/provenance、negative/security、determinism、resource、restart/rollback Gate 未完成前，不进入 lockfile/package/installer；
- 分发前完成 GPL、动态/进程边界、源码提供义务和 NOTICE 的法律审查。

当前尚未创建新的 Blender prototype receipt。若未来批准 6.1 provider，其输出只能是
`PrototypeObservation`；High/Low/UV/Cage/Bake 的产品对象仍由 Rust Runtime 在独立 contract、
strict readback、同 candidate/hash/lineage、quality 和 human/engine Gate 下验收。任何外部 DCC
输出都不能直接推进 Stage、confirm、version 或 export。

## 6.1 Blender 固定 revision 与刀类内部原型边界（2026-08-29）

本节是 ADR-0030 的实现级补充。本轮只完成资料与边界审计，不下载、安装或执行
Blender/插件；`isolated-prototype` 只描述一个未来可由单独批准开启的内部实验环境，
不改变产品采用状态。

### 固定版本、语言和许可证事实

当前规划的 Blender 固定 revision 是
[`72ccdd6e96ca119a1ffa3372559cc5654343b477`](https://github.com/blender/blender/commit/72ccdd6e96ca119a1ffa3372559cc5654343b477)，
其上游提交标题为 FBX `CastShadows` 修复，不是 release tag。该 revision 的
[`BKE_blender_version.h`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/source/blender/blenkernel/BKE_blender_version.h)
声明 `BLENDER_VERSION 503`、patch `0`、cycle `alpha`，因此应记录为 Blender
5.3.0-alpha source snapshot，而不能把 Blender 4.5 LTS 手册或插件兼容性直接套在它上面。

官方 [`COPYING`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/COPYING)
说明 Blender 本体使用 GNU GPL，且除 GPL 外没有另一个 Blender 总许可证；官方
[`license` 页面](https://www.blender.org/about/license/)还说明公开发布的 Python
add-on script 必须采用 GPL-compatible 许可，而 artwork、图片、电影、`.blend` 数据
的权利要逐项确认。故“内部可运行”不等于“可以把 Blender/plugin 链接、复制或嵌入
产品”；改写算法/数据语义仍需保持 clean-room，不复制 GPL 源码、Python、资产或
内部对象布局，最终分发边界交法务复核。

这不是一个可当作 Rust crate 的小工具：固定 source 的
[`CMakeLists.txt`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/CMakeLists.txt)
要求 CMake 3.21，并声明 GCC 14/Clang 17 等编译器门槛、Python/Python security、
可选 `WITH_HEADLESS`、OpenSubdiv、QuadriFlow、Manifold、OpenVDB、MaterialX、Embree、
Cycles、MikkTSpace、meshoptimizer 等依赖。[
`BMesh CMakeLists.txt`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/source/blender/bmesh/CMakeLists.txt)
列出 bevel、bridge、dissolve、extrude、inset、mirror、subdivide、symmetrize 等大批
C++ operator、内部拓扑查询/遍历和 Eigen/TBB/Bullet 等依赖；[
`depsgraph`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/source/blender/depsgraph/CMakeLists.txt)
与 [`nodes`](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/source/blender/nodes/CMakeLists.txt)
还分别包含 copy-on-write、animation/modifier/pose 求值和 Geometry/Shader/Function
节点生成/执行。结论是：Rust 重建的难点是语义、拓扑、求值图、宿主状态与依赖收敛的
组合，而不是把几组 API 名称翻译成 Rust。

### 刀类能力矩阵

| 能力 | 固定 Blender 隔离原型可做什么 | 许可证/宿主/无头与 I/O 风险 | 产品结论、Rust 真值与退出条件 |
|---|---|---|---|
| BMesh/硬表面 | 用内置 BMesh、Edit Mesh、Modifier 快速试验刀身、护手、槽、倒角、桥接、法线和非破坏顺序；官方 API 说明 BMesh 提供连接关系及 split/separate/collapse/dissolve 等编辑操作（见 [`BMesh API`](https://docs.blender.org/api/main/bmesh.html)）。 | 本体 GPL；C++ BMesh 由 `bpy`/data-block/operator context 绑定。`-b/--background` 可无 UI 执行（见 [`command line`](https://docs.blender.org/manual/en/4.5/advanced/command_line/arguments.html)），但 background 不等于已证明的无图形 `WITH_HEADLESS` 构建，也不消除 Python、插件、路径和浮点/线程差异。输入为受限临时 `.blend`/嵌入 GLB+typed 参数，输出临时 mesh/GLB/AOV。 | `AuthoringMesh`、稳定语义 ID、原子事务、Modifier/Evaluation Graph、严格回读必须 Rust-owned；禁止复制 BMesh handles/context。Rust fixture 与两次回放在同一宿主通过后，停用该原型；原型输出永不推进 candidate/Stage。 |
| High/雕刻/Multires/Subdivision | 用 Multires/Sculpt 观察刀刃圆滑、刻痕、磨损和细节层级；Multires 可在不同级别 sculpt，且可用 viewport level 作 Low、render level 作 High（见 [`Multires`](https://docs.blender.org/UATEST/manual/en/4.5/modeling/modifiers/generate/multiresolution.html)）。 | OpenSubdiv/OpenVDB 等可选依赖各有许可与平台差异；brush、sculpt session、modifier order、视图级别和资源峰值影响结果。输入高模/多级网格/固定 brush 参数，输出暂存 High/displacement/normal；不把 `.blend` 或 brush library 当资产真值。 | 十天只做 bounded high evaluator/细节 proposal；High topology、crease、位移 hash、预算与回读由 Rust 验证。完整 Sculpt parity 延后；Rust High fixture 与独立人审通过后移除原型。 |
| Low/Retopo | 内置 Quad Remesh/Poly Build/overlay/snapping 试验低模轮廓、feature locks 和人工对应；官方手册明确 Quad Remesh 受 seed 影响，自动拓扑不是最终变形拓扑（见 [`Retopology`](https://docs.blender.org/manual/en/4.5/modeling/meshes/retopology.html)）。 | QuadriFlow 编译选项和其自身许可证/solver 需另审；人工 UI 操作不适合稳定 headless replay。输入 High、目标面数、seams/锁定区域/seed，输出只作 draft Low、对应关系和诊断。 | 可让 Blender/QuadriFlow 提供 `LowDraft` 观察；可编辑 Low、High→Low correspondence、语义 Part 和人审记录必须 Rust-owned。固定 seed 仍不代表艺术质量；Rust draft/negative fixture 通过后，退出所有外部 retopo lane。 |
| UV | 内置 Unwrap/SLIM/Pack Islands 试验 seams、镜像/堆叠、UDIM、texel density 和 stretch；可观察其 UV face-corner 输出。 | UV 结果受 seams、版本、seed、margin、色彩/单位与插件宿主影响；输入 mesh+seams+UDIM/密度，输出 UV 坐标、atlas、stretch 诊断到临时文件。 | `HeroUvLayout`、稳定 island/face-corner 索引、密度/stretch 门和 CAS receipt 必须 Rust-owned；xatlas 仍只是候选研究，任何第三方 UV 输出不能成为 authored Low 或 export 真值。 |
| Bake/Cage | Cycles Selected-to-Active、Multires bake、normal/AO/curvature/thickness/ID 和 cage 误差可做视觉/流程 prototype；官方说明 bake 需要 UV、目标 Image Texture/Color Attribute，Selected-to-Active 依赖 ray distance/extrusion/cage，手工 cage 要求相同拓扑的面数与顺序（见 [`Cycles baking`](https://docs.blender.org/UATEST/manual/en/4.5/render/cycles/baking.html)）。 | 受 render settings、设备、CPU 内存（每对象有固定 footprint）、margin、色彩管理和 image codec 影响；`-b` 可无 UI，但不能宣称跨平台字节确定。输入 High/Low/cage/UV/固定 bake settings，输出暂存 PNG/EXR/纹理与 ray diagnostics。 | Rust 必须拥有 cage/ray miss-skew-intersection 诊断、MikkTSpace 约定、map byte/decoded readback、PBR 通道和 provenance；当前仅 `mikktspace@0.3.0` accepted。两个同 host fresh scratch 回放和引擎回读均通过后退出 Blender bake；不能用美观截图代替门。 |
| 材质/PBR | Principled BSDF/Shader Editor、通道打包、磨损层、节点连接和 AOV 预览可回答刀身金属/涂层/edge wear 的工作流问题；官方 Principled 以 OpenPBR 为基础并兼容 Disney/Standard Surface（见 [`Principled`](https://docs.blender.org/manual/en/4.5/render/shader_nodes/shader/principled.html)）。 | Blender 节点/data-block、图片路径、OCIO 配置和 renderer/device 绑定；输出是临时预览/贴图/graph 描述，不是产品材质状态。官方 Node Wrangler 源码是 GPL-2.0-or-later、`support: OFFICIAL`、导入 `bpy`，只能作为宿主 UX 参考。 | Rust `MaterialLayerGraph`、PBR slot/色彩策略、纹理 provenance 与 AOV/readback 是唯一真值；不复制 Node Wrangler/shader code。native graph fixture+decoded map gate 通过即移除插件/Blender。 |
| FPS/检视 | 用 Camera、collection、约束、固定镜头测试持刀、inspect、ADS、刀刃反光和 socket framing；只作固定视图观察。 | 视图/约束/父子 data-block、hand asset、插件 modal UI 难以无头复现。MESHmachine、HardOps/Boxcutter、DECALmachine 等成熟硬表面工具多为商业 vendor terms；DECALmachine 官方文档只支持 Blender 4.3–5.1，5.2 alpha 已属 experimental，而固定 host 是 5.3 alpha。输入临时 scene+camera/socket，输出 preview/AOV/package 草案。 | `FpsPresentationPackage`、camera/occlusion/ADS/inspect 参数、hand/socket/engine readback 必须 Rust-owned；商业插件只可在有授权的本地手工 prototype，永不进包/lockfile。Rust FPS fixture+独立人审/引擎门通过后立即移除。 |
| 动画 | 内置 Armature、Action/F-Curve、NLA、约束和 shape keys 试验刀柄/刀刃层级、idle/inspect/slash/stab clip、socket 与事件；官方说明 Action 存储 F-Curves，NLA 可作 strips（见 [`Actions`](https://docs.blender.org/manual/en/4.4/animation/actions.html)），Bake Action 会将约束/driver 等最终运动逐帧写成 keyframes（见 [`NLA bake`](https://docs.blender.org/manual/en/4.5/editors/nla/editing/strip.html)）。 | action slot/driver/constraint/NLA heuristics、父子关系和每帧 bake 会改变输入/输出；Rigify 为官方 bundled GPL-2.0-or-later add-on，但 feature set 可载入外部 rig，不能作为产品依赖。输入 armature/actions/constraints/keyframes，输出临时 clip/GLB/FBX/帧。 | Rust 只实现刀类刚性 hierarchy、socket、clip/event map、固定采样和导出回读；完整角色 rig、shape-key/NLA 编辑器 deferred。Rust clip fixture 与引擎 readback 通过后移除 Rigify/动画原型。 |
| Geometry Nodes/NURBS | Geometry Nodes 可原型化槽纹、重复齿、实例、属性传递与有限 procedure；NURBS 可参考刀背/刃线曲线。官方 Geometry Nodes Bake 明确存储在 `.blend`/磁盘且不是 interchange，版本兼容不保证（见 [`Geometry Nodes`](https://docs.blender.org/manual/en/4.5/modeling/geometry_nodes/index.html) 与 [`Bake`](https://docs.blender.org/manual/en/4.5/modeling/geometry_nodes/geometry/operations/bake.html)）；NURBS 控制点/权重是数学曲线（见 [`NURBS structure`](https://docs.blender.org/manual/en/4.5/modeling/curves/structure.html)）。 | Nodes 构建含 Python discovery 与大型 C++ geometry/depsgraph/field/instance 依赖；NURBS 全 editor、trim、continuity、surface/B-rep 远超十天。输入只允许预声明 typed graph/curve，输出 evaluated mesh/attributes/curve diagnostics。 | Rust 仅收敛刀类 `ModifierGraph`/bounded node subgraph 与可重复曲线 tessellation；不实现通用 Geometry Nodes/NURBS parity，不把 node bake 或 `.blend` cache 当真值。 |

### 官方/成熟插件处置矩阵

| 插件/来源 | 许可证与宿主事实 | 隔离 prototype 结论 | Rust 迁移/退出 |
|---|---|---|---|
| [Node Wrangler fixed source](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/scripts/addons_core/node_wrangler/__init__.py) | GPL-2.0-or-later，`support: OFFICIAL`，依赖 `bpy`，source 标 Blender 5.0；不能从标记推断对 5.3-alpha 的兼容。 | 可用于材质节点 UX/快捷操作观察；不得接收 caller Python 或写 Runtime。 | 映射为 `MaterialLayerGraph@1` translator/validator 和固定 PBR enums；不复制脚本，native graph fixture 通过后移除。 |
| [Rigify fixed source](https://raw.githubusercontent.com/blender/blender/72ccdd6e96ca119a1ffa3372559cc5654343b477/scripts/addons_core/rigify/__init__.py) | GPL-2.0-or-later，`support: OFFICIAL`，bundled，依赖 `bpy`/文件与 feature-set 加载。 | 只做刀类 rigid/hand rig 工作流观察；外部 feature set、角色 rig 和文件访问全部 quarantine。 | 映射为 `WeaponRig@1` 的刚性 socket、clip、event map；不搬 rig Python/骨骼资产，native clip/engine gate 后移除。 |
| [Magic UV Extensions](https://extensions.blender.org/add-ons/magic-uv/) | 官方 Extensions 页面标 GPL-2.0-or-later、Blender 4.2+、limited support；历史上曾 bundled。 | 可观察 UV 操作 UX，不当作稳定 solver；精确版本与 5.3-alpha host 仍需单独固定。 | 仅把 seam/island/packing 语义转成 `HeroUvLayout@1`；Rust validator 取代插件。 |
| [RetopoFlow](https://github.com/CGCookie/retopoflow/blob/master/blender_manifest.toml) | manifest 标 `SPDX:GPL-2.0-or-later`、min Blender 4.2；官方 docs 又称 code GPL 3.0 且 non-code assets 不同，存在版本/资产许可冲突。 | 只能研究操作/交互；在冲突解决前不下载、不执行、不复制源码/非代码资产，也不列为 adopted。 | 只提取 `LowDraft@1` 行为合同、对应关系和人审；Rust authored Low 通过后删除研究缓存。 |
| [UVPackmaster](https://uvpackmaster.com/doc3/blender/3.0.6/10-uvpackmaster-setup/) | addon 与 engine 分开许可；engine 需单独 EULA/安装，官方文档列 Windows/Linux 64-bit，版本必须匹配。 | 禁止作为可分发依赖；若未来有授权，只能离线手工比较 packing/密度，不能把安装脚本或 engine 放入产品。 | `HeroUvLayout` 只实现自有 deterministic packer/diagnostics；EULA engine 没有 Rust vendor/fallback 路径。 |
| [Zen UV Checker](https://extensions.blender.org/add-ons/zenuvchecker/) | 官方页面标 GPL-3.0-or-later、Blender 4.2+，需 Files permission；它是 checker，不等于完整 Zen UV。 | 可研究 checker 的 stretch/UV 可视化；权限、版本和 GPL 均不适合作为 Runtime 插件。 | 映射为 Rust UV diagnostics/AOV；不复制贴图、脚本或完整 Zen UV。 |
| [MESHmachine](https://machin3.io/MESHmachine/docs/) / [HardOps](https://hardops-manual.readthedocs.io/en/latest/installation/) / [Boxcutter](https://boxcutter-manual.readthedocs.io/en/latest/installation/) | 成熟硬表面商业工具，官方发布渠道是 vendor/marketplace；未找到可供本产品再分发的 SPDX 源码许可。 | 仅在另有商业授权、固定版本/host 的人工内部试验；不执行市场安装脚本，不把授权推定为可嵌入。 | 把 bevel/fillet/fuse/boolean/modal intent 写成 Rust typed operators/transactions；native hard-surface fixtures 通过即退出。 |
| [DECALmachine](https://machin3.io/DECALmachine/docs/installation/) | 商业 vendor terms；当前文档支持 Blender 4.3–5.1，5.2 alpha 仅 experimental，advanced decal/trim/bake 还依赖 Pillow。 | 固定 5.3-alpha 不在官方支持范围，不能作为本轮 prototype provider。 | 只借鉴 decal/wear layer 语义，Rust 实现 typed mask/material layer；不携带 decal mesh、Pillow、插件或资产。 |

“官方/成熟”只表示来源或市场成熟度，不代表 GPL 例外、可再分发或固定 5.3-alpha
兼容。任何插件都必须单独记录 exact version/source hash、许可证文本、Python/engine
依赖、宿主权限和移除 receipt；缺一项就保持 `reference-only`。

### 原型宿主、确定性、输入输出和退出

未来若 ADR-0030 单独批准 prototype，必须使用 exact source SHA、固定编译器/CMake
flags、完整依赖 SBOM 和离线 scratch。`-b/--background` 只表示无 UI 的命令行模式；
是否使用 `WITH_HEADLESS` 必须另行固定，不能用“headless”字样掩盖 Python 或插件能力。
每个 job 只接受 closed typed scene/mesh/material/bake/animation contract、内嵌或 hash
绑定的授权 fixture、固定 seed/frame/thread/CPU/color config；不得接受用户/Codex Python、
add-on zip、任意路径、URL、环境变量或网络服务。输出只能是 scratch 中的 GLB/FBX、PNG/AOV、
clip、诊断和 content-free receipt；不得直接写 Runtime SQLite/CAS/Stage。

确定性门至少包括同一宿主 fresh scratch 两次运行的字节 hash，以及独立的 semantic
readback（Part/拓扑/UV/tangent/PBR/socket/animation）；若 CPU/GPU、线程、OCIO、插件或
图像 codec 导致差异，标 `NOT_PROVEN` 并 fail closed。产品侧仍须重新解析、单位/finite/
预算/拓扑/通道/引擎回读，原型结果只能作为 `PrototypeObservation`，不能推进
candidate、confirm、version 或 export。

每个原型有独立 `prototype_id`、输入/输出 hash、许可证/依赖 receipt、资源与负向测试、
退出条件。对应 Rust capability 完成 typed contract、同 fixture differential、确定性
回放、strict readback、独立人审和引擎门后，关闭 provider、删除本地 plugin/binary/cache
（保留 hash-only 研究 receipt及失败原因），不得静默 fallback；许可证/版本/资源任一失败
即直接移除或保留 `rejected` 说明，不能将失败结果提升为产品能力。

### 把 Blender 插件逐层迁移为 Rust 能力

1. **许可和范围**：先区分 Blender/官方 GPL、第三方混合许可、商业 EULA；只提取公开
   行为与输入输出，不复制代码、资产、Python、`.blend` 或内部对象布局。
2. **行为盘点**：把 operator/menu/hotkey/modal workflow 记录为 capability；列出
   输入、输出、side effect、宿主 API、文件/网络权限和失败语义。
3. **合同先行**：设计 ForgeCAD Schema、closed enum/预算、canonical hash、provenance、
   负向错误码和 removal receipt；先有 contract/test 再写实现。
4. **数据模型解耦**：data-block/BMesh handle/context/undo 改为稳定 ID 的
   `AuthoringMesh`、`Part`、`MaterialZone`、`Socket`、`Action`；evaluated mesh 只作可丢弃投影。
5. **算法分层**：纯 Rust 做拓扑、UV、ray/bake、PBR/色彩和刀类动画；已 accepted 的
   `mikktspace`/Manifold 也只能在受限 Worker 内使用，其他库/插件保持 research-only。
6. **事务化执行**：把插件连续热键/模态操作变成 preview→prepare→commit/rollback 的
   原子 journal；Runtime 是唯一写者，插件/外部进程无 Store/CAS 访问。
7. **求值与验证**：固定 seed、CPU/thread、边界条件和资源；同 fixture 做 Blender
   shadow 与 Rust differential，比较语义几何/UV/PBR/animation，而非截图或 `.blend` hash。
8. **包装与退出**：为 Rust Worker 生成 LICENSE/NOTICE/SBOM/provenance/signature、
   build cohort 和 capability manifest；Rust gate 全绿后停止 Blender provider，并保留
   可审计 hash/差异报告。差异未解时保持 Rust `NOT_PROVEN`，不得用 Blender 作为产品 fallback。

典型映射为：Node Wrangler → `MaterialLayerGraph@1`；QuadriFlow/RetopoFlow →
`LowDraft@1`；Magic UV/UVPackmaster → `HeroUvLayout@1`；Rigify → `WeaponRig@1`；
DECALmachine → typed decal/wear material masks。映射的是可验证语义，不是源码翻译。

### 绝不允许的整仓/产品内嵌路径

- 不整仓 clone、vendoring 或链接 Blender；不把 Blender binary、Python bundle、`.blend`、
  plugin zip/cache、安装脚本、动态 loader、BlenderMCP/socket、任意 Python/JS 带入 Runtime、
  active Skill、lockfile、安装包或 CAS。
- 不让插件直接访问 SQLite/CAS/Stage，不让外部 `.blend`、插件输出或第二 renderer 成为
  authored High/Low/UV/Bake/PBR/animation truth，不以“内部使用”规避 GPL/EULA/NOTICE/SBOM
  与源码提供义务。
- 不因插件“能跑”、单张截图、`.blend` 保存成功或 Blender 自评通过而跳过 Runtime strict
  readback、确定性、资源/安全、独立人审和目标引擎回读。

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
