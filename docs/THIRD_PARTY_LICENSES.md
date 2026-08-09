# ForgeCAD License / SBOM Ledger

版本：2026-08-09
状态：当前依赖账本 + MVP 评估候选；候选不是已采用依赖

MCP002 建立的最小依赖账本继续作为基础；MCP005 起每次 adoption 增量更新。

## 当前产品依赖摘要

| Component | Role | License/source | Status |
|---|---|---|---|
| Rust standard library | Runtime implementation | Rust project terms | tracked |
| Tauri 2 | Desktop Viewer shell | MIT/Apache-2.0 | tracked in Cargo lock |
| rusqlite bundled SQLite | Runtime store | MIT | tracked |
| React/Vite | Viewer build | MIT | tracked in npm lock |
| Three.js | Viewer renderer capability | MIT | tracked；MCP008 GLB canvas focused evidence PASS |

当前产品不包含模型 SDK/权重、远程 3D 服务、Blender/FreeCAD MCP、Python CAD 插件或 DCC sidecar。

## MVP approved-for-evaluation（未采用）

| Candidate | License 初筛 | Task | Decision required before dependency change |
|---|---|---|---|
| image-rs/image | MIT OR Apache-2.0 | MCP005 | exact version、features、LICENSE hash、decoder security/limits、SBOM |
| gltf-rs/gltf | MIT OR Apache-2.0 | MCP007 | exact version、external URI policy、malicious GLB、SBOM |
| Manifold | Apache-2.0 | MCP007 | exact revision/binding、transitives、FFI/package、topology benchmark |
| xatlas | MIT | MCP008 | exact revision、vendoring/build、determinism benchmark；MVP 当前使用 product-owned bounded UV mapping，未安装 |
| gltf-rs/mikktspace | MIT/Apache-2.0 初筛 | MCP008 | verify exact repository/license files、golden tangent |
| Khronos glTF-Validator | Apache-2.0 | MCP008 | pinned tool artifact、transitives、report normalization |
| glTF-Transform | MIT | MCP009 | dev-only Node boundary、lockfile/SBOM、determinism |

每项只有在 `docs/evidence/adoption/<project>/<revision>.yaml` 为 `approval: accepted` 后才进入 Cargo/npm/installer。采用时必须把精确版本、license files hash、transitive SBOM 和最终 binary/package 重新回填本文。

## 明确不采用到 MVP Runtime

- Blender MCP、FreeCAD MCP、CadQuery/build123d MCP：任意 Python/文件/OS 权限；
- TripoSR/Hunyuan3D/远程 image-to-3D：模型权重、GPU/网络 Provider、隐私和许可证边界；
- 未固定 revision 的 GitHub Skill/prompt/asset pack。

“免费”或“开源”不等于可分发。资产仍逐项记录作者、source ID/URL、retrieved_at、SHA-256、SPDX、license text hash、修改和允许用途。
