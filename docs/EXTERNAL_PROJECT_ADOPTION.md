# 外部项目、Blender 与 GitHub 采用清单

版本：2026-08-09
状态：MVP 采用决策；FGC-MCP010A 只重排目标；当前没有新增 `accepted` 项、依赖或 AssetPack

## 1. 采用规则

外部仓库只能以四种身份进入：

1. **Library**：链接到受限 Core/Worker；
2. **Tool/Worker**：独立进程、固定输入输出、签名和资源预算；
3. **Asset**：逐资产许可证、hash、作者和来源回执；
4. **Reference only**：只学习算法或交互，不复制代码/资产。

每项必须通过：维护活跃度、许可证/例外、依赖 SBOM、恶意输入、确定性、资源上限、平台打包、性能、替代/退出策略和 Benchmark。禁止整仓复制、自动运行安装脚本、拉取模型权重、执行 arbitrary Python/JavaScript、在 Runtime 内起不受控网络服务或让第三方格式成为第二真值。

采用状态只允许：`approved-for-evaluation | accepted | deferred | reference-only | rejected`。只有 `accepted` 且有精确 revision receipt 的项目才能改 lockfile/安装包。本文件当前没有 `accepted` 项。

## 2. MVP approved-for-evaluation

| 项目 | 可能用途 | 许可证初筛 | 决策 | 首个任务 / Gate |
|---|---|---|---|---|
| [image-rs/image](https://github.com/image-rs/image) | PNG/JPEG decode/admission | MIT/Apache-2.0 | approved-for-evaluation | MCP005；关闭 default features、只开 PNG/JPEG、decoder limits、恶意图片 |
| [gltf-rs/gltf](https://github.com/gltf-rs/gltf) | Rust GLB strict readback | MIT OR Apache-2.0 | approved-for-evaluation | MCP007；禁外部 URI、buffer/image/size 上限 |
| [Manifold](https://github.com/elalish/manifold) | robust mesh boolean/manifold | Apache-2.0 | approved-for-evaluation | MCP010D；v3.5.2/full revision、C API/FFI、面数/时间/内存/拓扑/source IDs/removal |
| [xatlas](https://github.com/jpcy/xatlas) | UV unwrap/pack | MIT | approved-for-evaluation | MCP010E；determinism、seam/overlap、跨平台 |
| [mikktspace Rust](https://github.com/gltf-rs/mikktspace) | tangent generation | MIT/Apache-2.0 初筛 | approved-for-evaluation | MCP010E；精确许可证复核、与 Viewer/GLB golden 一致 |
| [Khronos glTF-Validator](https://github.com/KhronosGroup/glTF-Validator) | GLB 交付验证 | Apache-2.0 | approved-for-evaluation | MCP010E/F；恶意 GLB、版本 pin、JSON 报告归一 |
| [glTF-Transform](https://github.com/donmccurdy/glTF-Transform) | GLB inspection/优化 | MIT | approved-for-evaluation-as-dev-tool | MCP009；Node 只在构建/测试，不能写 Runtime 真值 |
| [img2threejs](https://github.com/img2threejs/img2threejs) | 分阶段 image → typed spec → procedural review 的工作流思想 | Apache-2.0 | approved-for-evaluation / first-party reimplementation | MCP006；仅学习 staged passes、detail inventory、per-region confidence 和 side-by-side review；不安装其 Python/TypeScript skill，不把 Three.js/JS 作为 Runtime 真值 |

“许可证初筛”不是法律批准。当前仍没有 `accepted` 第三方 3D compiler/UV/render dependency；MCP008 的 UV/tangent/software render 由 product-owned bounded implementation 提供。Luna 可以做隔离 benchmark，但只有 `accepted` receipt 才能改 lockfile；distribution legal review、最终二进制 SBOM 和签名仍在 MCP012/013。

## 3. Deferred / benchmark-first

| 项目 | 用途 | 决策理由 |
|---|---|---|
| [meshoptimizer](https://github.com/zeux/meshoptimizer) | LOD/mesh compression | deferred；先证明正确 mesh/readback，再优化 |
| [MaterialX](https://github.com/AcademySoftwareFoundation/MaterialX) | 材质交换 | deferred；MVP 只实现 glTF metallic-roughness 子集 |
| [OpenColorIO](https://github.com/AcademySoftwareFoundation/OpenColorIO) | 色彩管理 | deferred；先固定 sRGB/linear 基线，跨 renderer 时再引入 |
| [truck](https://github.com/ricosjp/truck) | Rust B-rep/NURBS CAD kernel | benchmark-first；能力和依赖面超过首个 mesh vertical slice |
| [Parry](https://github.com/dimforge/parry) | collision/query | deferred；MCP010F 可先用 product-owned bbox explosion，采用 Parry 仍需独立 receipt |
| OpenImageIO/OpenEXR/OpenCV/Embree/Filament/KTX/Basis | 高级图像、AOV、renderer、压缩 | deferred；包体/codec/插件/第二 renderer 风险 |
| OpenUSD | 场景交换 | post-MVP reference；不进入 V1 真值 |

## 4. Reference-only / rejected 项目

| 项目 | 学习点 | 不直接采用原因 |
|---|---|---|
| [Blender](https://github.com/blender/blender) | Data-block、Modifier/Geometry Nodes、Principled、UV/Bake、AOV、OCIO、Asset Browser、Outliner | reference-only；GPL 分发边界、任意 Python、`.blend` 不能成为产品真值 |
| [BlenderMCP](https://github.com/ahujasid/blender-mcp) | DCC tool choreography | rejected for MVP；可执行任意 Blender Python、socket/网络资产，已公开报告 unrestricted `exec()` 风险 |
| [img2css](https://github.com/javierbyte/img2css) | 像素采样、颜色/轮廓预览和轻量 reference visualizer 思想 | BSD-3-Clause；CSS box-shadow/base64 输出只用于离线预览，不能进入 GeometryProgram、不能执行任意 JS/HTML |
| [build123d](https://github.com/gumyr/build123d) / [CadQuery](https://github.com/CadQuery/cadquery) / FreeCAD MCP | MCP 工具粒度、CAD 操作暴露 | reference-only；任意 Python/文件/OS 与工程 CAD 状态边界不匹配 |
| TripoSR/Hunyuan3D/其他 image-to-3D | benchmark、候选导入合同 | rejected for MVP；不内置权重/GPU/远程 3D Provider，未来另立 ADR |

Reference only 必须保存研究链接和自研设计理由，但不复制源文件、提示词包或素材。

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
