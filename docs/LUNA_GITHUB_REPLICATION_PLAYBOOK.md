# Luna GitHub 受控复刻操作手册

> 商业武器开源研究新增边界：只 clean-room 吸收算法/数据结构思想或固定 revision 的 typed Worker source；不得把外部 half-edge handle、脚本、DCC 状态、模型输出或引擎项目变成 ForgeCAD 真值。选型见 `FPS_HERO_WEAPON_PRODUCTION_RESEARCH_20260826.md`。

> 2026-08-26 商业采用优先级：Manifold=isolated Boolean；OpenSubdiv=CPU High evaluator 候选；QuadriFlow=license/transitive audit 后仅 draft retopo；xatlas=UV draft；Embree=ray kernel；MaterialX=typed graph semantics；OIIO/OCIO=隔离 image/color；meshoptimizer=approved LOD/package；Basis/KTX2=texture delivery；glTF Transform=固定导出 allowlist；Khronos Validator=format only。任何整仓、动态插件、外部 solver/PATH 或脚本入口均不得进入 Runtime。

> 2026-08-25 执行补充：Native High 本轮是 ForgeCAD-owned source 实现和合同同步，不是 GitHub 复刻或三方采用。Luna 可并行学习方法，但不得把视频/教程/仓库结论提升为 Runtime capability、active Skill 或商业质量 PASS。

版本：2026-08-25
状态：用户已明确授权为商业武器路线复制、下载、研究并在许可证允许范围内使用 GitHub/其他网站的开源项目；后续对本文列明候选进入隔离 adoption cache 不需要逐仓再次确认。授权不等于 `accepted`：QuadriFlow、Embree、OpenImageIO、meshoptimizer、glTF Transform、Basis/KTX2 等仍须先固定 revision、抓取实际 LICENSE/NOTICE、生成 transitive SBOM、静态审查，再允许构建 benchmark；只有 receipt 通过的窄模块才可选择性 vendor/link 到 fixed Worker，不能整仓变成 Runtime/Skill。所有未通过候选均不进入 lockfile/package/Runtime/active Skill。商业采用顺序见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。

## 1. 目的与边界

Luna 可以从下表的冻结 revision 读取、下载并在隔离研究缓存中保存**指定源文件**，以学习模块划分、API 形态、测试思路和数据合同。复刻的目标是 ForgeCAD 自有的 typed 设计能力，不是把上游程序、Python 脚本、MCP 插件或构建系统搬进产品。

这份授权不改变以下事实：

- `forgecad-runtime` 仍是唯一永久状态写者；
- MCP 仍是薄 `stdio` adapter，Viewer 仍是只读；
- 当前只有 `mikktspace@0.3.0` 是已接受的外部依赖；
- Manifold 已完成固定 revision 的 product-owned isolated Worker adoption，`boolean@1` 当前开放同一 Part 的 bounded union/difference/intersection；MaterialX、OpenSubdiv、xatlas、glTF-Validator、OpenColorIO 和其余项目尚未进入 lockfile、安装包或 Runtime；
- Blender 官方 source 与 headless 路径固定为 `reference-only / unavailable-for-product`；ADR-0028 只保留非产品威胁模型与许可证研究，不能进入 lockfile、安装包、active Skill 或 Runtime allowlist；
- 当前唯一 `in_progress` 是 `FGC-MCP010F`。本手册只准备后续设计和评估，不得借此跳过现有质量真值或改写 `QUALITY_TARGET_NOT_MET`。

商业级能力的产品真值必须留在 ForgeCAD：typed Form/AuthoringMesh/High/Low/UV/Cage/Bake/Material/FPS/LOD/Engine contracts、固定 Worker、GLB strict readback、candidate/hash/lineage/CAS、Runtime 单写者和 Stage/confirm/version/export/restart gates。上游仓库只能帮助设计 Schema、测试和 clean-room rewrite，不能直接提供 Runtime 状态、active Skill、第二 writer 或视觉/商业质量结论。Blender 与 Substance Designer/Painter 均为学习参考；binary、工程文件、material graph、插件、脚本、权重和会话状态都不可进入产品。

## 2. 允许的上游快照

| 项目 | 冻结 revision | 许可证 | ForgeCAD 学习/复刻目标 | 允许进入的下一站 |
|---|---|---|---|---|
| [Blender official](https://github.com/blender/blender) | `72ccdd6e96ca119a1ffa3372559cc5654343b477` | GPL-2.0-or-later | Modifier/Depsgraph/Action 的数据与求值分层 | **仅 reference；headless 产品路径 unavailable，不缓存/复制/链接/执行 source** |
| [build123d](https://github.com/gumyr/build123d) | `ef48b98af7780028e015d9f079d8ccc01d894696` | Apache-2.0 | BuildPart/BuildSketch、操作与 topology 的职责划分 | 静态研究缓存；再以 Rust typed JSON 重写为 Parametric Design Kit |
| [BlenderMCP](https://github.com/ahujasid/blender-mcp) | `3ab892510cc0e5435ba5e611c01fb1021fbde8de` | MIT | scene inspect、截图回看、tool receipt 的可观察性 | 静态研究缓存；再定义 ForgeCAD 自有 read-only observe/visual-evidence 合同 |
| [CadQuery](https://github.com/CadQuery/cadquery) | `d6729f51bf1ed183f110aacdbc6238e4a5110c96` | Apache-2.0 | Workplane/Sketch/selector/assembly 的参数化表达方式 | 静态研究缓存；再定义 bounded macro/schema 和 Rust Worker 实现 |
| [Manifold](https://github.com/elalish/manifold) | `969b1417afdee87dbc6147cf676bc04799418ec2` | Apache-2.0 | robust manifold mesh、C API 边界、拓扑测试 | **accepted：product-owned isolated C API/FFI Worker；启用同一 Part union/difference/intersection；通用 mesh 仍隔离研究** |
| [MaterialX](https://github.com/AcademySoftwareFoundation/MaterialX) | `a7b2d60aa682656b6fed72f760685612aa3a87c6` | Apache-2.0 | material document/node/definition、look/material graph 和 PBR 映射 | 静态研究缓存；再定义 MaterialZone/PBR translator 的数据合同 |
| [OpenSubdiv](https://github.com/PixarAnimationStudios/OpenSubdiv) | `4951f30c00f395aa831a9fc42577cc28ce46fa81` | Tomorrow Open Source Technology License 1.0 | CPU subdivision/refinement API shape | 静态研究缓存；TSL 法务、CPU worker、资源/确定性 Gate |
| [xatlas](https://github.com/jpcy/xatlas) | `f700c7790aaa030e794b52ba7791a05c085faf0c` | MIT | chart segmentation、seam/atlas packing | 静态研究缓存；typed Worker、transitive SBOM、确定性/资源 Gate |
| [Khronos glTF-Validator](https://github.com/KhronosGroup/glTF-Validator) | `bcd52cc4ba5f333b2999a58f67cc05ddf28b4fb1` | Apache-2.0 | 外部 GLB validation report | 静态研究缓存；bytes-only wrapper、external-resource denial、report normalization |
| [OpenColorIO](https://github.com/AcademySoftwareFoundation/OpenColorIO) | `c52966a6677723d5bd2dbef0ccec3fed9cbc3790` | BSD-3-Clause | scene-linear/display transform semantics | 静态研究缓存；explicit config provenance、deterministic transform Gate |

上述 revision、许可证文件 Git blob 和候选路径记录在 `docs/evidence/adoption/<project>/<revision>.yaml`。Blender 官方 source receipt 的 `approval: research-authorized` 只表示“可研究”；不得再生成可用于产品晋级的 headless Worker `approved-for-evaluation` receipt。

### 2.1 尚未冻结的新评估候选

下列项目只获得产品层面的 `approved-for-evaluation`，尚未成为“允许的上游快照”。在取得完整 commit、许可证文件 blob、候选文件范围和 research receipt 前，只允许读取公开 metadata/LICENSE，不允许下载源码、构建、运行或修改产品依赖。

| 项目 | 目标能力 | 获取前必须确定 | 当前状态 |
|---|---|---|---|
| QuadriFlow | automatic quad draft for Low/retopo | 完整 commit、README MIT 标签与 `LICENSE.txt` BSD-style+enhancement grant 差异、Eigen/solver 依赖、`BUILD_FREE_LICENSE=ON` 路径、外部 PATH/solver 排除 | approved-for-evaluation / snapshot blocked |
| Embree | High→Low cage/bake ray kernel | 完整 commit、Apache/SPDX、CPU ISA/打包范围、ray API 与测试文件 | approved-for-evaluation / snapshot blocked |
| OpenImageIO | bake/map image IO、mipmap、channel processing | 完整 commit、codec/传递依赖、最小 feature set、恶意图像测试范围 | approved-for-evaluation / snapshot blocked |
| meshoptimizer | authored LOD 后的 attribute-aware optimization | 完整 commit、MIT license blob、stable API/test 文件、experimental API 排除表 | approved-for-evaluation / snapshot blocked |

OpenSubdiv、xatlas、MaterialX、OpenColorIO 和 glTF-Validator 已有冻结研究快照，但仍只允许在其既有 receipt 范围内做静态研究；新的商业质量用途必须追加 receipt，不得沿用旧 receipt 冒充新 benchmark 或 accepted adoption。

## 3. 强制流程

1. **冻结来源**：先用 GitHub connector 读取 repository metadata、LICENSE、目标文件和完整 commit；不得以移动分支名作为复刻来源。
2. **先写研究 receipt**：记录 source URL、revision、license、许可证文件 blob、拟取文件、预期能力、禁止能力和退出计划。receipt 不能包含本机用户名、绝对路径、用户参考或 secret。
3. **只获取精确文件**：保存到 `FORGECAD_ADOPTION_CACHE/<project>/<revision>/` 的研究缓存。不得把整仓 clone、下载档案、构建产物或外部依赖放入当前产品树。
4. **静态审查**：逐文件检查许可证头、依赖树、网络、文件系统、动态代码、子进程、遥测、模型调用和平台构建脚本；发现未声明能力即停止。
5. **选择落点**：每个文件只能被标为 `reference`, `rewrite`, `quarantine`, `isolated-evaluation` 或 `rejected`。原样复制不是默认落点。
6. **先写 ForgeCAD 合同和测试**：产品功能必须先有 Schema、预算、negative tests、lineage/readback 和 removal plan；不得从上游接口直接暴露任意表达式或脚本。
7. **隔离验证**：只有 Manifold 类 library 候选可在无网络、固定输入输出、资源上限的 Worker benchmark 中编译。上游 Python、Blender binary/addon、socket server、自动下载和上游安装脚本一律不执行；ADR-0028 只保留非产品威胁模型与许可证研究，不授权 headless binary/Recipe/Python bundle 执行。
8. **单独接受**：只有完整 receipt 的 `approval: accepted`、SBOM、许可证审查、恶意输入、确定性、资源和平台 Gate 均通过后，才允许修改 lockfile、包或 Worker。否则只保留 research receipt 和删除/回滚结果。

## 4. 每个项目的文件级约束

| 项目 | 可选研究文件范围 | 必须复刻为 ForgeCAD 自有能力 | 永不直接进入 Runtime 的内容 |
|---|---|---|---|
| build123d | `build_part.py`、`build_sketch.py`、`operations_part.py`、`topology/**` 的结构与测试思路 | `ParametricDesignKit@1` 的 macro/parameter/source-map schema，及 bounded Rust lowering | Python runtime、OCCT binding、Jupyter/VTK、import/export 脚本 |
| CadQuery | `cq.py`、`sketch.py`、`selectors.py`、`assembly.py`、`occ_impl/shape_protocols.py` 的 API 设计 | Workplane/selector 意图到 typed Part/Operator 的受限映射 | 任意 CadQuery script、OCP/FreeCAD binding、plugin/GUI 代码 |
| BlenderMCP | `src/blender_mcp/server.py`、`telemetry*.py`、`addon.py` 仅供安全与协议研究 | `scene_observe_get`、render/evidence receipt、超时/错误归一化的自有合同 | `exec()`、Blender Python、socket server、遥测、资产 API、远程 host、`.blend` 状态 |
| Manifold | `bindings/c/**`、`include/manifold/**`、必要 `src/**`、LICENSE | 受限 C FFI request/result、mesh budget、source-ID/readback、fallback；已 vendored 62 files | 自动 CMake 构建、WASM/Python bindings、任意上游脚本、任意 mesh Boolean |
| MaterialX | `MaterialXCore/{Document,Element,Node,Material,Definition,Look,Value,Variant}` 的数据模型 | MaterialZone/PBR graph interchange translator 的自有 schema | CMake、Viewer、Graph Editor、shader generation/render backends、Python/JS bindings |

所有原样保留的上游文本都必须连同原始许可证/NOTICE 一起留在研究缓存或受控 quarantine，保留 source URL、revision、文件路径、原始 hash、修改说明和 removal plan。进入产品树时优先重写；若未来必须 vendoring，需另立 accepted receipt 并同步 `THIRD_PARTY_LICENSES.md`、SBOM 和最终包检查。

### 4.1 Native High / GLB / durable 的产品边界（不是上游采用）

ForgeCAD-owned `HighMeshArtifact@1`、`HighMeshArtifactGlb@1` 和 `NativeHighDurable*` 不属于 GitHub adoption。Worker 只接受闭合 typed request，在 bounded one-shot process 中生成稳定 Part/name/lineage 的 source artifact；GLB sibling 只做 embedded-only lowering 和 strict local readback；Runtime durable prepare/get 只在 exact durable AuthoringMesh binding 后双回放 High/GLB、校验 cohort/bytes/hash，并由 Runtime/Store/CAS 记录 derived artifact/link，getter 重哈希且不写状态。

这条 source/structural/durable slice 不能被 Luna 解释为第三方依赖、active Skill、Commercial High Gate 或视觉 PASS。无论研究或实现代码是否可编译，都不能放宽 `registered=false`、integration=`unavailable`、no arbitrary script/network/path、no Stage/confirm/version/export；未有独立 receipt 的正向 GLB/CAS/Store/package/restart/visual/human/engine 结果统一保持 `NOT_RUN` 或 `NOT_PROVEN`。

### 4.2 Blender headless threat-model archive（non-product research only）

本节只保留被拒绝的产品方案所暴露的安全、许可证和资源约束，不能作为 Tool/Worker 晋级入口：

- 产品不发布、不签名、不调用 Blender binary、Recipe 或 Python bundle；若在隔离研究环境阅读这些边界，只能形成非产品审计笔记。
- 复用 strict `WorkerRequest@1` / `WorkerResponse@1` envelope；初始 operation 仅允许 `render_fixed@1` 与 `high_low_bake@1`，payload 只接受闭合 Scene/Material/Bake/Render contract。
- Runtime 是 binary selector、scope validator、CAS owner、SQLite/candidate/version/Stage/rollback 唯一写者；Worker 无数据库/CAS写权限，`.blend` 仅为临时中间缓存。
- 禁止 Codex/Skill/用户 Python、addon、`.blend` 宏、`exec`/`eval`、动态 subprocess、网络/DNS/socket、URL、任意文件路径、环境变量和 secret；只允许 Runtime 创建的隔离 scratch。
- 初始评估沿用产品 bounded request/response、stderr、CPU、wall-clock、memory、GPU、texture、triangle 和 deterministic replay ceilings；超时必须 kill/reap，失败不落 CAS，重启不恢复 Blender session。
- Blender 不产生产品 render/evidence receipt。High/Low/UV/Cage/Bake 只能由独立 ForgeCAD contract、strict readback、same candidate/hash/lineage、Stage@3 quality、人审和 engine Gate 证明。
- `GPL-2.0-or-later` 对应源码提供、NOTICE、逐依赖 SPDX/SBOM、签名/provenance 和动态/进程边界法律审查全部是前置 Gate；任何一项缺失即 `CAPABILITY_UNAVAILABLE` 或拒绝。
- 当前产品状态固定为 `UNAVAILABLE_FOR_PRODUCT`；没有 binary/Python bundle 下载、执行、lockfile/package/installer/Runtime allowlist 或 active Skill 变更。

## 5. 给 Luna 的执行目标

```text
在不改变当前唯一 in_progress FGC-MCP010F 的前提下，按 LUNA_GITHUB_REPLICATION_PLAYBOOK.md 对指定上游仓库执行受控研究。每次只处理一个项目和一个明确模块：固定完整 commit，先写 research-authorized receipt，再从 GitHub 取得精确源文件到隔离缓存，进行许可证/依赖/动态代码/网络/文件系统审查。把学习结果重写为 ForgeCAD 自有 Schema、Rust Worker 或只读 MCP/Viewer contract；不得运行上游安装脚本、Python、Blender binary/addon、Substance plugin/SDK、socket server、模型权重或自动下载。Blender 与 Substance 产品能力固定 `UNAVAILABLE_FOR_PRODUCT`；不得修改 lockfile、安装包、active Skill 或 Runtime allowlist。
```

## 6. 失败与清理

- revision、许可证、文件 hash 或上游权限不明确：标为 `rejected`，删除缓存副本并保留原因；
- 静态审查发现动态执行、未授权网络/文件系统或无法界定依赖：只保留最小审计摘要，不保留可执行副本；
- benchmark 不稳定、资源超限或 lineage 不完整：不推广，保留 `approval: rejected` 和 fallback；
- Blender 研究若触及 binary/addon/Python 执行、GPL/source-offer、动态代码、网络或产品依赖边界：立即停止，保留最小非产品审计摘要；能力保持 `unavailable-for-product`，ForgeCAD Native Worker 不受影响；
- 上游不再可用：现有 receipt 继续引用固定 revision；不得悄悄切换到新分支或新 release。

研究缓存和 quarantine 不是产品模块、Skill 或证据 PASS。任何删除都遵守 `DEPRECATED_ISOLATION_PLAN.md`，不得在脏 worktree 直接删除未知内容。
