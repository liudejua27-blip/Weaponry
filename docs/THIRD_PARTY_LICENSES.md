# ForgeCAD License / SBOM Ledger

> **Weaponry P0 override (2026-08-29):** 本文只有在与 `docs/WEAPONRY_CROSSFIRE_PRODUCT_CONSTITUTION.md` 和 ADR-0029 一致时才具有当前执行权。ForgeCAD 在本文中解释为 Weaponry 的 Rust Runtime lineage；当前唯一产品主线是由 Codex 生成、修改、验证并交付高质量穿越火线非功能性游戏武器。通用 3D、机器人和原创科幻示例仅作 fixture/历史能力，不得抢占本月主线。 本文中所有 2026-08-28 及更早的“当前”“下一原子”“唯一 `in_progress`”和工具/Schema 数量语句均按历史 cohort 解释，不得覆盖 `WPN-*` successor queue。

> 2026-08-26 现行 source 面为 **527 schemas / 115 read + 87 write = 202 tools**。Manifold 固定 revision/Apache-2.0 的 vendored slice 可 feature-gated 编译链接；OpenSubdiv、QuadriFlow、xatlas、Embree、OpenImageIO、meshoptimizer、glTF Transform、Basis/KTX2 仍须分别经过 fixed-revision、许可证、SBOM、确定性、资源和 removal Gate，未标记 `accepted` 前不得进入产品真值。下文旧计数仅作历史 cohort。

> 2026-08-26 研究提醒：OpenSubdiv 为 Tomorrow OSL 1.0；QuadriFlow README/license/transitive Eigen 路径需逐项核验并使用 free-license build；OIIO codecs、KTX/Basis components、生成式 3D 模型权重均需独立许可证/SBOM。未固定 revision 和 receipt 前不得进入发布包。详见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 采用账本说明：OpenSubdiv、QuadriFlow、xatlas、Embree、MaterialX、OpenImageIO、OpenColorIO、meshoptimizer 与 glTF-Validator 目前最多是 `approved-for-evaluation`/`research-authorized`，不是可分发 dependency，不应进入最终安装包 SBOM 的 accepted 集合。ForgeCAD-owned Low/Hero UV/Cage-Bake source seam 不构成这些项目的 adoption receipt。只有完成固定 revision、LICENSE/NOTICE、transitive SBOM、negative/security、determinism、resource、package 和 removal Gate 后，才能由独立 accepted receipt 改变本账本。

这些候选在商业武器资产链中的用途与首个 Gate 由 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md` 统一定义；本文件只记录实际许可证/SBOM/adoption 状态，不把计划项目写成当前依赖。

版本：2026-08-26
状态：当前依赖账本 + MCP010D/E source-focused 采用回执；Manifold 仅以固定 revision vendored C API/Worker 方式进入产品，xatlas/Validator/OpenSubdiv/MaterialX/OpenColorIO 和其他候选仍未采用。本轮新增的固定 revision 研究 receipt 只证明许可证/候选文件/静态能力审查，未修改 lockfile、安装包或 Runtime allowlist。用户已授权 Luna 对 build123d、BlenderMCP、CadQuery、Manifold、MaterialX 进行冻结 revision 的选择性源文件研究；另有 Ponytail 的 accepted first-party workflow rewrite。

MCP002 建立的最小依赖账本继续作为基础；MCP005 起每次 adoption 增量更新。

当前 accepted 产品切片只有：固定 revision 的 bounded Manifold same-Part Boolean Worker，以及 `mikktspace@0.3.0` source-focused tangent Worker。其余表中的 `research-authorized` 或 `approved-for-evaluation/snapshot-blocked` 项目全部 `NOT_IN_RELEASE`，不得出现在 active Runtime allowlist、安装包依赖摘要或发布签名清单中。

## 当前产品依赖摘要

| Component | Role | License/source | Status |
|---|---|---|---|
| Rust standard library | Runtime implementation | Rust project terms | tracked |
| Tauri 2 | Desktop Viewer shell | MIT/Apache-2.0 | tracked in Cargo lock |
| rusqlite bundled SQLite | Runtime store | MIT | tracked |
| React/Vite | Viewer build | MIT | tracked in npm lock |
| Three.js | Viewer renderer capability | MIT | tracked；MCP008 GLB canvas focused evidence PASS |

当前产品不包含模型 SDK/权重、远程 3D 服务、Blender/FreeCAD MCP、Python CAD 插件、DCC sidecar、Pi Agent runtime、Omniverse Kit SDK、OpenUSD runtime、FreeCAD/build123d/CadQuery runtime、Trimesh、MaterialX runtime 或 TRELLIS/Hunyuan3D 权重。

### 明确排除的 DCC 运行依赖

| Component | License/terms | Product status | Boundary |
|---|---|---|---|
| Blender / Blender headless / `bpy` / BlenderMCP | GPL-2.0-or-later / MIT bridge | `EXCLUDED / reference-only / unavailable-for-product` | 不下载、不执行、不打包、不进入 lockfile、Runtime allowlist、CAS/Stage 真值或 fallback |
| Substance Designer/Painter / SDK / project graph | commercial terms；资产许可逐项审 | `EXCLUDED / reference-only / unavailable-for-product` | 只学习 layer/bake/channel workflow；不执行、不打包、不保存工程 graph 为真值 |
| Maya / Maya Python / plugins / scene files | commercial terms | `EXCLUDED / reference-only / unavailable-for-product` | 不执行、不打包、不允许 Python/plugin/scene 成为 Worker 或 Runtime 输入 |

上述排除不因本机已安装软件、GitHub 示例、研究 receipt 或 source PASS 而改变；只有 ForgeCAD-owned typed Worker 能进入生产路径。

## 受控研究快照（非产品依赖）

| 项目 | 冻结 revision | 许可证 | receipt | 状态 |
|---|---|---|---|---|
| build123d | `ef48b98af7780028e015d9f079d8ccc01d894696` | Apache-2.0 | `docs/evidence/adoption/build123d/ef48b98af7780028e015d9f079d8ccc01d894696.yaml` | research-authorized |
| BlenderMCP | `3ab892510cc0e5435ba5e611c01fb1021fbde8de` | MIT | `docs/evidence/adoption/blender-mcp/3ab892510cc0e5435ba5e611c01fb1021fbde8de.yaml` | research-authorized |
| CadQuery | `d6729f51bf1ed183f110aacdbc6238e4a5110c96` | Apache-2.0 | `docs/evidence/adoption/cadquery/d6729f51bf1ed183f110aacdbc6238e4a5110c96.yaml` | research-authorized |
| Manifold | `969b1417afdee87dbc6147cf676bc04799418ec2` | Apache-2.0 | `docs/evidence/adoption/manifold/969b1417afdee87dbc6147cf676bc04799418ec2.yaml` | **accepted：product-owned isolated Worker，同一 Part union/difference/intersection** |
| MaterialX | `a7b2d60aa682656b6fed72f760685612aa3a87c6` | Apache-2.0 | `docs/evidence/adoption/materialx/a7b2d60aa682656b6fed72f760685612aa3a87c6.yaml` | research-authorized |
| MaterialX | `7b64921ef1d42f2d57871e9d2c43dc11f041f26b` | Apache-2.0 | `docs/evidence/adoption/materialx/7b64921ef1d42f2d57871e9d2c43dc11f041f26b.yaml` | research-authorized；更新快照，未采用 |
| OpenSubdiv | `4951f30c00f395aa831a9fc42577cc28ce46fa81` | Tomorrow Open Source Technology License 1.0 | `docs/evidence/adoption/opensubdiv/4951f30c00f395aa831a9fc42577cc28ce46fa81.yaml` | research-authorized |
| xatlas | `f700c7790aaa030e794b52ba7791a05c085faf0c` | MIT | `docs/evidence/adoption/xatlas/f700c7790aaa030e794b52ba7791a05c085faf0c.yaml` | research-authorized |
| Khronos glTF-Validator | `bcd52cc4ba5f333b2999a58f67cc05ddf28b4fb1` | Apache-2.0 | `docs/evidence/adoption/gltf-validator/bcd52cc4ba5f333b2999a58f67cc05ddf28b4fb1.yaml` | research-authorized |
| OpenColorIO | `c52966a6677723d5bd2dbef0ccec3fed9cbc3790` | BSD-3-Clause | `docs/evidence/adoption/opencolorio/c52966a6677723d5bd2dbef0ccec3fed9cbc3790.yaml` | research-authorized |
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

## 商业模块候选账本（仅审计/排队）

下表是商业 High/Retopo/UV/Cage-Bake/Surface/LOD/Engine 链的候选，不是当前安装包依赖。除已有 bounded Manifold 与 `mikktspace@0.3.0` 受限采用外，其余均保持未采用；任何候选都必须先封装为 ForgeCAD-owned、离线、签名、确定性的 `ForgeCadModule@1`，不能以插件、联网服务或脚本进入 Runtime。

| 候选 | 初筛许可证 | 预期 ForgeCAD module | 当前状态 |
|---|---|---|---|
| Manifold | Apache-2.0 | bounded Native High Boolean Worker | accepted 仅限固定 revision/同一 Part `boolean@1`；通用 mesh `NOT_RUN` |
| OpenSubdiv | TSL-1.0（需法务确认） | Native High subdivision/crease Worker | research-authorized；未进入 accepted SBOM，determinism/resource/package `NOT_RUN` |
| QuadriFlow | `LICENSE.txt` 为 BSD-3-Clause 风格并附 enhancement grant；README 标成 MIT，标签冲突需法务确认 | Retopology draft Worker | snapshot-blocked；固定 revision/transitive SBOM/`BUILD_FREE_LICENSE=ON`/许可证冲突未闭合，只能生成 draft |
| xatlas | MIT | Hero UV draft/packing Worker | research-authorized；未安装，当前 Hero UV 是 ForgeCAD structural/source slice |
| Embree | Apache-2.0 | Cage-Bake ray Worker | approved-for-evaluation；CPU feature、恶意 mesh、miss/skew、resource `NOT_RUN` |
| MaterialX | Apache-2.0 | Material Layer translator | research-authorized；仅 typed subset 研究，不执行 shader/runtime |
| OpenImageIO (OIIO) | Apache-2.0 | Surface texture/map I/O Worker | approved-for-evaluation；codec、内存、色彩/通道、package `NOT_RUN` |
| OpenColorIO (OCIO) | BSD-3-Clause | Color policy Worker | research-authorized；固定 config/provenance/跨平台确定性 `NOT_RUN` |
| meshoptimizer | MIT | LOD optimization Worker | approved-for-evaluation；Part/UV/tangent/material/socket/silhouette no-regression `NOT_RUN` |
| Khronos glTF Validator | Apache-2.0 | delivery/engine report adapter | research-authorized；不能替代 Runtime GLB readback 或 `EngineValidationReceipt@1` |
| glTF Transform | MIT | fixed allowlist Packaging Worker / dev oracle | approved-for-evaluation；禁止用户脚本，优化前后 semantic diff 与 deterministic replay `NOT_RUN` |
| Basis Universal / KTX-Software/KTX2 | Basis Universal Apache-2.0 但含第三方模块；KTX-Software 需逐组件审计，`lib/etcdec.cxx` 含特殊 Ericsson 许可 | texture compression Worker | benchmark-first；normal/data 优先 UASTC、color profile、4-texel alignment、完整 mip、source/decoded/compressed hash 与 package Gate `NOT_RUN` |

OpenSubdiv 的准确许可证名称是 `Tomorrow Open Source Technology License 1.0`，不是 Apache-2.0；其文本以 Apache-2.0 为基础但修改了商标条款。任何文档、SBOM 或发布清单必须使用上游固定 revision 的实际 LICENSE 文本与 hash，不能用简称推断兼容性。

`ForgeCadModule@1` 采用记录必须同时给出 `schema_refs`、`operator_refs`、有限 `budget`、正/负 `fixture_refs`、LICENSE/NOTICE hashes、transitive SPDX `sbom_sha256`、source/build `provenance`、签名、`module_sha256`、`contract_set_sha256` 和 input/output hashes，并声明 `network=false`、`dynamic_plugin=false`、`script=false`、`direct_db_write=false`、`direct_cas_write=false`。没有固定 revision、完整许可证文本、SBOM、恶意输入/资源/确定性 benchmark、包验证、签名和 removal receipt，账本状态不得改成 `accepted`，也不能改变当前 **515 schemas / 28 operator entries / 111 read + 83 opt-in write = 194 tools** 或 Hero UV durable 的 structural/source 解释。
