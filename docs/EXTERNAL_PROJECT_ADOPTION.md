# 外部项目、Blender 与 GitHub 采用清单

版本：2026-08-07
状态：候选研究；没有任何项目因出现在本文而被采用或打包

## 1. 采用规则

外部仓库只能以四种身份进入：

1. **Library**：链接到受限 Core/Worker；
2. **Tool/Worker**：独立进程、固定输入输出、签名和资源预算；
3. **Asset**：逐资产许可证、hash、作者和来源回执；
4. **Reference only**：只学习算法或交互，不复制代码/资产。

每项必须通过：维护活跃度、许可证/例外、依赖 SBOM、恶意输入、确定性、资源上限、平台打包、性能、替代/退出策略和 Benchmark。禁止整仓复制、自动运行安装脚本、拉取模型权重、执行 arbitrary Python/JavaScript、在 Runtime 内起不受控网络服务或让第三方格式成为第二真值。

## 2. P0/P1 候选

| 项目 | 可能用途 | 许可证初筛 | 建议 | Gate |
|---|---|---|---|---|
| [Khronos glTF-Validator](https://github.com/KhronosGroup/glTF-Validator) | GLB 交付验证 | Apache-2.0 | P0 library/tool | 恶意 GLB、版本 pin、报告归一 |
| [xatlas](https://github.com/jpcy/xatlas) | UV unwrap/pack | MIT | P0 isolated library | deterministic、seam/overlap、跨平台 |
| [mikktspace Rust](https://github.com/gltf-rs/mikktspace) | tangent generation | MIT/Apache-2.0 | P0 library | 与 glTF/renderer golden 一致 |
| [Manifold](https://github.com/elalish/manifold) | robust mesh boolean | Apache-2.0 | P0 worker/library | 资源上限、拓扑/readback |
| [MaterialX](https://github.com/AcademySoftwareFoundation/MaterialX) | 材质图交换与验证 | Apache-2.0 | P0 contract/reference | 只采用受限节点子集 |
| [OpenPBR](https://github.com/AcademySoftwareFoundation/OpenPBR) | Principled PBR 语义参考 | Apache-2.0 | P0 reference | glTF lowering 定义 |
| [OpenColorIO](https://github.com/AcademySoftwareFoundation/OpenColorIO) | scene-linear/color transforms | BSD-3-Clause | P0 worker/library | 配置打包、golden image |
| [OpenImageIO](https://github.com/AcademySoftwareFoundation/OpenImageIO) | 图像读写/纹理/bake 工具 | Apache-2.0 core，插件另审 | P1 isolated worker | 插件、编解码漏洞、SBOM |
| [glTF-Transform](https://github.com/donmccurdy/glTF-Transform) | GLB pipeline/inspection | MIT | P0 tool/reference | Node runtime 与确定性边界 |
| [meshoptimizer](https://github.com/zeux/meshoptimizer) | LOD/mesh compression | MIT | P1 library | 误差、readback、兼容性 |
| [KTX-Software](https://github.com/KhronosGroup/KTX-Software) | KTX2 容器与工具 | Apache-2.0 为主，逐文件复核 | P1 worker/tool | 例外、引擎 roundtrip |
| [Basis Universal](https://github.com/BinomialLLC/basis_universal) | 纹理压缩 | Apache-2.0 为主，含第三方 | P1 isolated worker | 质量/通道/许可证 |
| [Khronos glTF Sample Viewer](https://github.com/KhronosGroup/glTF-Sample-Viewer) | PBR 渲染参考 | Apache-2.0 | reference/benchmark | 不形成第二 UI 真值 |
| [pixelmatch](https://github.com/mapbox/pixelmatch) | 固定图像差异 | ISC | P0 test tool | 阈值和色彩管理 |
| [OpenCV](https://github.com/opencv/opencv) | 轮廓/特征/图像指标 | Apache-2.0 | P0/P1 worker | 构建裁剪和资源上限 |
| [Embree](https://github.com/RenderKit/embree) | CPU ray evidence | Apache-2.0 | P1 renderer candidate | 平台/性能/包体 |
| [Filament](https://github.com/google/filament) | headless/参考 renderer | Apache-2.0 | P1 benchmark first | 避免第二材质/渲染真值 |
| [OpenEXR](https://github.com/AcademySoftwareFoundation/openexr) | HDR/AOV | BSD-3-Clause | P1 worker | 编解码安全 |
| [Material Maker](https://github.com/RodZill4/material-maker) | 程序化材质思想 | MIT | reference first | 图节点不能原样执行 |
| [Open Image Denoise](https://github.com/RenderKit/oidn) | 可选离线降噪 | Apache-2.0，含预训练滤波器 | P2 optional | 披露权重、不能当质量证明 |
| [OpenUSD](https://github.com/PixarAnimationStudios/OpenUSD) | 未来场景/装配交换 | modified Apache-2.0 | P2 reference | 不进入 P0 真值链 |

“许可证初筛”不是法律批准。`FGC-MCP012` 必须记录精确 revision、LICENSE/NOTICE、transitive dependency、构建产物和最终分发方式。

## 3. Reference-only 项目

| 项目 | 学习点 | 不直接采用原因 |
|---|---|---|
| [Blender](https://www.blender.org/) | Data-block、Modifier/Geometry Nodes、Principled、UV/Bake、AOV、OCIO、Asset Browser、Outliner | GPL 分发边界、任意 Python、`.blend` 不能成为产品真值 |
| [BlenderMCP](https://github.com/ahujasid/blender-mcp) | DCC tool choreography | 常见实现可执行任意 Blender Python/外部资产 |
| [img2threejs](https://github.com/img2threejs/img2threejs) | image-to-code-to-Three.js 迭代工作流 | Three.js/JS 不能成为几何真值，网页相似不等于可编辑高质量资产 |
| FreeCAD MCP / build123d MCP | MCP 工具粒度、CAD 操作暴露 | 产品偏工程 CAD，且任意 Python/CAD 内核状态边界不匹配 |
| TripoSR/Hunyuan3D/其他 image-to-3D | benchmark、候选导入合同 | P0 不内置权重/GPU/远程 3D Provider；未来另立 ADR |

Reference only 必须保存研究链接和自研设计理由，但不复制源文件、提示词包或素材。

## 4. 材质与 HDRI 资产候选

- [Poly Haven](https://polyhaven.com/)：逐资产 CC0 回执；API/站点代码许可证与资产许可证分开处理；
- [ambientCG](https://ambientcg.com/)：逐资产 CC0 回执；保留原始 ID、hash、分辨率、通道、单位和下载时间；
- 其他 marketplace/社区包：默认不自动导入，先确认每个资产及再分发条款。

即使是 CC0，也必须记录 source URL/ID、retrieved_at、SHA-256、作者、许可证文本 hash、物理尺寸、色彩空间和通道语义。远程链接不能成为版本真值，入库后只引用 CAS hash。

## 5. Blender worker 边界

若后续引入 Blender：

- 仅由产品发布并签名的固定 Recipe 启动 headless worker；
- 输入是受限 Scene/Material/Bake/Render 合同，输出是 CAS 工件和 receipt；
- 禁止 Codex/Skill/用户传 Python、addon、`.blend` 宏、任意文件路径或网络 URL；
- 独立临时目录、无网络、最小文件授权、CPU/GPU/内存/时间限制；
- `.blend` 只是中间缓存，不是项目真值；
- 产品必须提供无 Blender 的降级能力或清楚标记 unavailable；
- 分发前完成 GPL、动态/进程边界、源码提供义务和 NOTICE 的法律审查。

## 6. 采用记录模板

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
