# 外部项目、Blender 与 GitHub 采用清单

版本：2026-08-13
状态：MCP010E source-focused 采用决策；固定 `mikktspace@0.3.0` 已以受限 Worker Library 进入 Cargo.lock；xatlas、Manifold、Khronos Validator 和其他第三方仍未进入产品真值。ADR-0026 新增 Pi Agent、Omniverse Kit、OpenUSD、FreeCAD、build123d/CadQuery、BlenderMCP、Trimesh、MaterialX、TRELLIS.2/Hunyuan3D 的研究边界；用户另授权 Luna 对 build123d、BlenderMCP、CadQuery、Manifold、MaterialX 进行冻结 revision 的选择性源文件研究。除既有 `mikktspace@0.3.0` 外均未 adopted。

## 1. 采用规则

外部仓库只能以四种身份进入：

1. **Library**：链接到受限 Core/Worker；
2. **Tool/Worker**：独立进程、固定输入输出、签名和资源预算；
3. **Asset**：逐资产许可证、hash、作者和来源回执；
4. **Reference only**：只学习算法或交互，不复制代码/资产。

“受控源文件研究”不是第五种产品身份，而是 adoption 前的隔离流程：它只允许经过用户授权、冻结 revision、逐文件 receipt 的上游文件进入非产品研究缓存。它不改变 `Reference only` 的产品边界，也不等于 Library、Tool/Worker 或 Asset 已被采用。

每项必须通过：维护活跃度、许可证/例外、依赖 SBOM、恶意输入、确定性、资源上限、平台打包、性能、替代/退出策略和 Benchmark。禁止整仓复制、自动运行安装脚本、拉取模型权重、执行 arbitrary Python/JavaScript、在 Runtime 内起不受控网络服务或让第三方格式成为第二真值。

产品采用状态只允许：`approved-for-evaluation | accepted | deferred | reference-only | rejected`。研究 receipt 可以额外标为 `research-authorized`，但它不属于产品采用状态。只有作为依赖或二进制采用的 `accepted` 项目才能改 lockfile/安装包。本文件当前只有受限范围的 `mikktspace@0.3.0` 为 accepted dependency；`ponytail-preflight` 是另行记录的 accepted first-party workflow rewrite，不含上游代码或依赖。

## 2. MVP approved-for-evaluation

| 项目 | 可能用途 | 许可证初筛 | 决策 | 首个任务 / Gate |
|---|---|---|---|---|
| [image-rs/image](https://github.com/image-rs/image) | PNG/JPEG decode/admission | MIT/Apache-2.0 | approved-for-evaluation | MCP005；关闭 default features、只开 PNG/JPEG、decoder limits、恶意图片 |
| [gltf-rs/gltf](https://github.com/gltf-rs/gltf) | Rust GLB strict readback | MIT OR Apache-2.0 | approved-for-evaluation | MCP007；禁外部 URI、buffer/image/size 上限 |
| [Manifold](https://github.com/elalish/manifold) | robust mesh boolean/manifold | Apache-2.0 | approved-for-evaluation | MCP010D；v3.5.2/full revision、C API/FFI、面数/时间/内存/拓扑/source IDs/removal |
| [xatlas](https://github.com/jpcy/xatlas) | UV unwrap/pack | MIT | approved-for-evaluation | MCP010E；determinism、seam/overlap、跨平台；当前不安装，产品使用 bounded triangle-chart packer |
| [mikktspace Rust](https://github.com/gltf-rs/mikktspace) | MikkTSpace tangent generation | MIT/Apache-2.0 | **accepted**（仅 MCP010E source-focused Worker） | 固定 0.3.0、源码 revision、crate/license/SBOM receipt、确定性/恶意输入/GLB handedness Gate；见 `docs/evidence/adoption/mikktspace/0.3.0.yaml` |
| [Khronos glTF-Validator](https://github.com/KhronosGroup/glTF-Validator) | GLB 交付验证 | Apache-2.0 | approved-for-evaluation | MCP010E/F；恶意 GLB、版本 pin、JSON 报告归一 |
| [glTF-Transform](https://github.com/donmccurdy/glTF-Transform) | GLB inspection/优化 | MIT | approved-for-evaluation-as-dev-tool | MCP009；Node 只在构建/测试，不能写 Runtime 真值 |
| [img2threejs](https://github.com/img2threejs/img2threejs) | 分阶段 image → typed spec → procedural review 的工作流思想 | Apache-2.0 | approved-for-evaluation / first-party reimplementation | MCP006；仅学习 staged passes、detail inventory、per-region confidence 和 side-by-side review；不安装其 Python/TypeScript skill，不把 Three.js/JS 作为 Runtime 真值 |

“许可证初筛”不是法律批准。当前只有 `mikktspace@0.3.0` 作为受限 tangent library 通过 source-focused receipt；UV atlas 仍是 ForgeCAD 自有的 512px bounded chart packer，xatlas 尚未安装，Manifold/Validator 也未进入产品包。distribution legal review、最终二进制 SBOM 和签名仍在 MCP012/013。

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
| Manifold | `research-authorized` | C API/FFI 的隔离 benchmark 设计 | 自动构建、直接启用 `boolean@1` 或写 Runtime state |
| MaterialX | `research-authorized` | MaterialZone/PBR graph translator 的自有 schema | shader/render/Viewer/JS/Python runtime 引入 |

研究副本只允许存放于受控 adoption cache 或 quarantine，不能提交为 active 模块。`vendored_files` 在这五份 receipt 中仍为空，表示本轮只完成研究快照和操作授权，尚未复制任何上游源码到产品树。

### 2.4 Ponytail 前置工作流重写（2026-08-13）

[`DietrichGebert/ponytail`](https://github.com/DietrichGebert/ponytail) 在固定
`2ed6c52c9d7e5e56942508591085fd45dea277d3` revision、MIT 许可证下被接受为
**workflow reference only**。ForgeCAD 只重写了“必要性 → 复用既有能力 → 最小
typed action”的决策顺序，落地为 first-party `ponytail-preflight@0.1.0` 和 MCP
session-order policy；详见其 receipt 与 `CODEX_PONYTAIL_PREFLIGHT_WORKFLOW.md`。

没有复制或运行上游 Source，没有修改 Cargo/npm lockfile，没有安装其 Node package、
lifecycle hook 或 MCP server。该 accepted receipt 不是可分发 third-party dependency，
也不把引用 Skill、bundle integrity 或会话顺序升级为几何/视觉质量通过。

## 3. Deferred / benchmark-first

| 项目 | 用途 | 决策理由 |
|---|---|---|
| [meshoptimizer](https://github.com/zeux/meshoptimizer) | LOD/mesh compression | deferred；先证明正确 mesh/readback，再优化 |
| [MaterialX](https://github.com/AcademySoftwareFoundation/MaterialX) | 材质交换 | deferred；MVP 只实现 glTF metallic-roughness 子集 |
| [OpenColorIO](https://github.com/AcademySoftwareFoundation/OpenColorIO) | 色彩管理 | deferred；先固定 sRGB/linear 基线，跨 renderer 时再引入 |
| [truck](https://github.com/ricosjp/truck) | Rust B-rep/NURBS CAD kernel | benchmark-first；能力和依赖面超过首个 mesh vertical slice |
| [Parry](https://github.com/dimforge/parry) | collision/query | deferred；MCP010F 可先用 product-owned bbox explosion，采用 Parry 仍需独立 receipt |
| OpenImageIO/OpenEXR/OpenCV/Embree/Filament/KTX/Basis | 高级图像、AOV、renderer、压缩 | deferred；包体/codec/插件/第二 renderer 风险 |
| [OpenUSD](https://github.com/PixarAnimationStudios/OpenUSD) | scene graph、layer、variant、reference、交换 | reference-only / post-MVP benchmark；只学习 SemanticSceneGraph/ReferenceCanvas 思想，不进入 V1 真值 |
| [Trimesh](https://github.com/mikedh/trimesh) | mesh analysis/repair/test reference | deferred / dev-reference；Python 库不进入 Runtime 真值，未来只能做离线验证或隔离工具 |
| [NVIDIA Omniverse Kit](https://docs.omniverse.nvidia.com/dev-guide/latest/kit-architecture.html) | extension/app composition、headless/UI split、USD-based workbench architecture | reference-only；不引入 Kit SDK、Carbonite、USD runtime 或插件系统 |
| Pi Agent | minimal linear agent harness、skills/extensions/prompt templates | reference-only；只学习 harness 形态，不把 Pi runtime 变成产品状态或 P0 dependency |

## 4. Reference-only / rejected 项目

| 项目 | 学习点 | 不直接采用原因 |
|---|---|---|
| [Blender](https://github.com/blender/blender) | Data-block、Modifier/Geometry Nodes、Principled、UV/Bake、AOV、OCIO、Asset Browser、Outliner | reference-only；GPL 分发边界、任意 Python、`.blend` 不能成为产品真值 |
| [BlenderMCP](https://github.com/ahujasid/blender-mcp) | DCC tool choreography | rejected for MVP；可执行任意 Blender Python、socket/网络资产，已公开报告 unrestricted `exec()` 风险 |
| [img2css](https://github.com/javierbyte/img2css) | 像素采样、颜色/轮廓预览和轻量 reference visualizer 思想 | BSD-3-Clause；CSS box-shadow/base64 输出只用于离线预览，不能进入 GeometryProgram、不能执行任意 JS/HTML |
| [FreeCAD](https://github.com/FreeCAD/FreeCAD) / FreeCAD MCP | Document、transaction、parametric recompute、workbench/undo | reference-only；不接入 FreeCAD MCP，不把工程 CAD 状态、任意 Python 或文件系统暴露给 Runtime |
| [build123d](https://github.com/gumyr/build123d) / [CadQuery](https://github.com/CadQuery/cadquery) | AI 友好的参数化 CAD API、OCCT/BREP modeling style | reference-only；当前只映射为 Parametric Design Kit typed JSON macro，不执行任意 Python/CadQuery script |
| TripoSR / [TRELLIS.2](https://github.com/microsoft/TRELLIS.2) / [Hunyuan3D](https://github.com/tencent-hunyuan/hunyuan3d-2.1) / 其他 image-to-3D | draft mesh、候选导入合同、PBR draft research | rejected for MVP / future opt-in research；不内置权重/GPU/远程 3D Provider，不能直接 confirm/export，未来另立 ADR |

普通 `Reference only` 必须保存研究链接和自研设计理由，但不复制源文件、提示词包或素材。第 2.3 节五个指定项目是唯一例外：Luna 可以依照受控复刻手册把精确文件放入研究缓存；它们仍不得进入 active 模块、Skill、Runtime、lockfile 或安装包。

## 5. 材质与 HDRI 资产候选

- [Poly Haven](https://polyhaven.com/)：逐资产 CC0 回执；API/站点代码许可证与资产许可证分开处理；
- [ambientCG](https://ambientcg.com/)：逐资产 CC0 回执；保留原始 ID、hash、分辨率、通道、单位和下载时间；
- 其他 marketplace/社区包：默认不自动导入，先确认每个资产及再分发条款。

即使是 CC0，也必须记录 source URL/ID、retrieved_at、SHA-256、作者、许可证文本 hash、物理尺寸、色彩空间和通道语义。远程链接不能成为版本真值，入库后只引用 CAS hash。

MCP005–009 首个机器人仍只使用 typed procedural/PBR values。MCP010E 计划由 Codex 一次性下载指定的 ambientCG `Metal010` 2K PNG、`Plastic006` 2K PNG 和 Poly Haven `Studio Small 03` HDRI 到本机 adoption cache，不调用 API。每项先固定下载文件 hash、CC0 license text hash、作者/source ID、通道/色彩空间、派生 Recipe 和 SBOM；原 ZIP 不进入 Git。只有逐资产 receipt 通过后，派生内容才能进入 first-party `forgecad-hard-surface-robot@1.0.0` 离线 AssetPack。

Runtime、Viewer 和安装器不得联网或接收素材 URL；远程链接不进入版本真值。010E 不实现通用 pack 安装/升级/撤销，这些属于 MCP012。

## 6. Blender worker 边界

若后续引入 Blender：

- 仅由产品发布并签名的固定 Recipe 启动 headless worker；
- 输入是受限 Scene/Material/Bake/Render 合同，输出是 CAS 工件和 receipt；
- 禁止 Codex/Skill/用户传 Python、addon、`.blend` 宏、任意文件路径或网络 URL；
- 独立临时目录、无网络、最小文件授权、CPU/GPU/内存/时间限制；
- `.blend` 只是中间缓存，不是项目真值；
- 产品必须提供无 Blender 的降级能力或清楚标记 unavailable；
- 分发前完成 GPL、动态/进程边界、源码提供义务和 NOTICE 的法律审查。

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
