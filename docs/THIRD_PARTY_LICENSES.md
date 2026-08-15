# ForgeCAD License / SBOM Ledger

版本：2026-08-13
状态：当前依赖账本 + MCP010D/E source-focused 采用回执；Manifold 仅以固定 revision vendored C API/Worker 方式进入产品，xatlas/Validator 和其他候选仍未采用。用户已授权 Luna 对 build123d、BlenderMCP、CadQuery、Manifold、MaterialX 进行冻结 revision 的选择性源文件研究；另有 Ponytail 的 accepted first-party workflow rewrite。

MCP002 建立的最小依赖账本继续作为基础；MCP005 起每次 adoption 增量更新。

## 当前产品依赖摘要

| Component | Role | License/source | Status |
|---|---|---|---|
| Rust standard library | Runtime implementation | Rust project terms | tracked |
| Tauri 2 | Desktop Viewer shell | MIT/Apache-2.0 | tracked in Cargo lock |
| rusqlite bundled SQLite | Runtime store | MIT | tracked |
| React/Vite | Viewer build | MIT | tracked in npm lock |
| Three.js | Viewer renderer capability | MIT | tracked；MCP008 GLB canvas focused evidence PASS |

当前产品不包含模型 SDK/权重、远程 3D 服务、Blender/FreeCAD MCP、Python CAD 插件、DCC sidecar、Pi Agent runtime、Omniverse Kit SDK、OpenUSD runtime、FreeCAD/build123d/CadQuery runtime、Trimesh、MaterialX runtime 或 TRELLIS/Hunyuan3D 权重。

## 受控研究快照（非产品依赖）

| 项目 | 冻结 revision | 许可证 | receipt | 状态 |
|---|---|---|---|---|
| build123d | `ef48b98af7780028e015d9f079d8ccc01d894696` | Apache-2.0 | `docs/evidence/adoption/build123d/ef48b98af7780028e015d9f079d8ccc01d894696.yaml` | research-authorized |
| BlenderMCP | `3ab892510cc0e5435ba5e611c01fb1021fbde8de` | MIT | `docs/evidence/adoption/blender-mcp/3ab892510cc0e5435ba5e611c01fb1021fbde8de.yaml` | research-authorized |
| CadQuery | `d6729f51bf1ed183f110aacdbc6238e4a5110c96` | Apache-2.0 | `docs/evidence/adoption/cadquery/d6729f51bf1ed183f110aacdbc6238e4a5110c96.yaml` | research-authorized |
| Manifold | `969b1417afdee87dbc6147cf676bc04799418ec2` | Apache-2.0 | `docs/evidence/adoption/manifold/969b1417afdee87dbc6147cf676bc04799418ec2.yaml` | **accepted：product-owned isolated Worker，同一 Part union/difference/intersection** |
| MaterialX | `a7b2d60aa682656b6fed72f760685612aa3a87c6` | Apache-2.0 | `docs/evidence/adoption/materialx/a7b2d60aa682656b6fed72f760685612aa3a87c6.yaml` | research-authorized |
| Ponytail | `2ed6c52c9d7e5e56942508591085fd45dea277d3` | MIT | `docs/evidence/adoption/ponytail/2ed6c52c9d7e5e56942508591085fd45dea277d3.yaml` | accepted workflow reference; no code/dependency |

`research-authorized` 只表示可在受控缓存中研究精确文件。它不是组件、transitive dependency、SBOM entry 或可分发代码；任何原样副本必须继续保留上游许可证和 provenance。Ponytail receipt 的 `accepted` 仅接受 ForgeCAD 自有的静态工作流重写，`vendored_files: []` 且没有第三方依赖、SBOM package 或可分发代码；只有 accepted dependency 才能改变本账本的“当前产品依赖摘要”。

## MVP approved-for-evaluation（未采用）

| Candidate | License 初筛 | Task | Decision required before dependency change |
|---|---|---|---|
| image-rs/image | MIT OR Apache-2.0 | MCP005 | exact version、features、LICENSE hash、decoder security/limits、SBOM |
| gltf-rs/gltf | MIT OR Apache-2.0 | MCP007 | exact version、external URI policy、malicious GLB、SBOM |
| Manifold | Apache-2.0 | MCP010D | fixed revision/vendor hash、C API/FFI、topology/readback、determinism、resource/ASan/UBSan、removal fallback；现为 bounded Worker adoption |
| xatlas | MIT | MCP008 | exact revision、vendoring/build、determinism benchmark；MVP 当前使用 product-owned bounded UV mapping，未安装 |
| gltf-rs/mikktspace 0.3.0 | MIT/Apache-2.0 | MCP010E | **accepted（受限 Worker）**；固定 revision、license/SBOM、确定性和 GLB handedness 回执见 `docs/evidence/adoption/mikktspace/0.3.0.yaml` |
| Khronos glTF-Validator | Apache-2.0 | MCP008 | pinned tool artifact、transitives、report normalization |
| glTF-Transform | MIT | MCP009 | dev-only Node boundary、lockfile/SBOM、determinism |

每项只有在 `docs/evidence/adoption/<project>/<revision>.yaml` 为 `approval: accepted` 后才进入 Cargo/npm/installer。采用时必须把精确版本、license files hash、transitive SBOM 和最终 binary/package 重新回填本文。

## 明确不采用到 MVP Runtime

- Blender MCP、FreeCAD MCP、CadQuery/build123d MCP：任意 Python/文件/OS 权限；
- TripoSR/TRELLIS.2/Hunyuan3D/远程 image-to-3D：模型权重、GPU/网络 Provider、隐私和许可证边界；
- Pi Agent、NVIDIA Omniverse Kit、OpenUSD、FreeCAD、build123d/CadQuery、Trimesh、MaterialX：当前只作为 reference-only/deferred 研究，不进入 package 或 SBOM；
- 未固定 revision 的 GitHub Skill/prompt/asset pack。

“免费”或“开源”不等于可分发。资产仍逐项记录作者、source ID/URL、retrieved_at、SHA-256、SPDX、license text hash、修改和允许用途。
