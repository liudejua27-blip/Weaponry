# Luna GitHub 受控复刻操作手册

版本：2026-08-13  
状态：用户已授权五个指定上游项目的受控研究和选择性源文件复刻；这不是依赖采用、Skill 安装或 Runtime 集成授权。

## 1. 目的与边界

Luna 可以从下表的冻结 revision 读取、下载并在隔离研究缓存中保存**指定源文件**，以学习模块划分、API 形态、测试思路和数据合同。复刻的目标是 ForgeCAD 自有的 typed 设计能力，不是把上游程序、Python 脚本、MCP 插件或构建系统搬进产品。

这份授权不改变以下事实：

- `forgecad-runtime` 仍是唯一永久状态写者；
- MCP 仍是薄 `stdio` adapter，Viewer 仍是只读；
- 当前只有 `mikktspace@0.3.0` 是已接受的外部依赖；
- `boolean@1` 仍 unavailable；Manifold、MaterialX 和其余四个项目尚未进入 lockfile、安装包或 Runtime；
- 当前唯一 `in_progress` 是 `FGC-MCP010F`。本手册只准备后续设计和评估，不得借此跳过现有质量真值或改写 `QUALITY_TARGET_NOT_MET`。

## 2. 允许的上游快照

| 项目 | 冻结 revision | 许可证 | ForgeCAD 学习/复刻目标 | 允许进入的下一站 |
|---|---|---|---|---|
| [build123d](https://github.com/gumyr/build123d) | `ef48b98af7780028e015d9f079d8ccc01d894696` | Apache-2.0 | BuildPart/BuildSketch、操作与 topology 的职责划分 | 静态研究缓存；再以 Rust typed JSON 重写为 Parametric Design Kit |
| [BlenderMCP](https://github.com/ahujasid/blender-mcp) | `3ab892510cc0e5435ba5e611c01fb1021fbde8de` | MIT | scene inspect、截图回看、tool receipt 的可观察性 | 静态研究缓存；再定义 ForgeCAD 自有 read-only observe/visual-evidence 合同 |
| [CadQuery](https://github.com/CadQuery/cadquery) | `d6729f51bf1ed183f110aacdbc6238e4a5110c96` | Apache-2.0 | Workplane/Sketch/selector/assembly 的参数化表达方式 | 静态研究缓存；再定义 bounded macro/schema 和 Rust Worker 实现 |
| [Manifold](https://github.com/elalish/manifold) | `969b1417afdee87dbc6147cf676bc04799418ec2` | Apache-2.0 | robust manifold mesh、C API 边界、拓扑测试 | 隔离 C API/FFI benchmark；通过 accepted receipt 前不得启用 boolean |
| [MaterialX](https://github.com/AcademySoftwareFoundation/MaterialX) | `a7b2d60aa682656b6fed72f760685612aa3a87c6` | Apache-2.0 | material document/node/definition、look/material graph 和 PBR 映射 | 静态研究缓存；再定义 MaterialZone/PBR translator 的数据合同 |

上述 revision、许可证文件 Git blob 和候选路径记录在 `docs/evidence/adoption/<project>/<revision>.yaml`。这些记录的 `approval: research-authorized` 明确表示“可研究”，不表示 `accepted`。

## 3. 强制流程

1. **冻结来源**：先用 GitHub connector 读取 repository metadata、LICENSE、目标文件和完整 commit；不得以移动分支名作为复刻来源。
2. **先写研究 receipt**：记录 source URL、revision、license、许可证文件 blob、拟取文件、预期能力、禁止能力和退出计划。receipt 不能包含本机用户名、绝对路径、用户参考或 secret。
3. **只获取精确文件**：保存到 `FORGECAD_ADOPTION_CACHE/<project>/<revision>/` 的研究缓存。不得把整仓 clone、下载档案、构建产物或外部依赖放入当前产品树。
4. **静态审查**：逐文件检查许可证头、依赖树、网络、文件系统、动态代码、子进程、遥测、模型调用和平台构建脚本；发现未声明能力即停止。
5. **选择落点**：每个文件只能被标为 `reference`, `rewrite`, `quarantine`, `isolated-evaluation` 或 `rejected`。原样复制不是默认落点。
6. **先写 ForgeCAD 合同和测试**：产品功能必须先有 Schema、预算、negative tests、lineage/readback 和 removal plan；不得从上游接口直接暴露任意表达式或脚本。
7. **隔离验证**：只有 Manifold 类 library 候选可在无网络、固定输入输出、资源上限的 Worker benchmark 中编译。Python、Blender addon、socket server、自动下载和上游安装脚本一律不执行。
8. **单独接受**：只有完整 receipt 的 `approval: accepted`、SBOM、许可证审查、恶意输入、确定性、资源和平台 Gate 均通过后，才允许修改 lockfile、包或 Worker。否则只保留 research receipt 和删除/回滚结果。

## 4. 每个项目的文件级约束

| 项目 | 可选研究文件范围 | 必须复刻为 ForgeCAD 自有能力 | 永不直接进入 Runtime 的内容 |
|---|---|---|---|
| build123d | `build_part.py`、`build_sketch.py`、`operations_part.py`、`topology/**` 的结构与测试思路 | `ParametricDesignKit@1` 的 macro/parameter/source-map schema，及 bounded Rust lowering | Python runtime、OCCT binding、Jupyter/VTK、import/export 脚本 |
| CadQuery | `cq.py`、`sketch.py`、`selectors.py`、`assembly.py`、`occ_impl/shape_protocols.py` 的 API 设计 | Workplane/selector 意图到 typed Part/Operator 的受限映射 | 任意 CadQuery script、OCP/FreeCAD binding、plugin/GUI 代码 |
| BlenderMCP | `src/blender_mcp/server.py`、`telemetry*.py`、`addon.py` 仅供安全与协议研究 | `scene_observe_get`、render/evidence receipt、超时/错误归一化的自有合同 | `exec()`、Blender Python、socket server、遥测、资产 API、远程 host、`.blend` 状态 |
| Manifold | `bindings/c/include/**`、`bindings/c/manifoldc.cpp`、边界测试和 C API 文档 | 受限 C FFI request/result、mesh budget、source-ID/readback、fallback | 自动 CMake 构建、WASM/Python bindings、任意上游脚本；未 accepted 的 boolean |
| MaterialX | `MaterialXCore/{Document,Element,Node,Material,Definition,Look,Value,Variant}` 的数据模型 | MaterialZone/PBR graph interchange translator 的自有 schema | CMake、Viewer、Graph Editor、shader generation/render backends、Python/JS bindings |

所有原样保留的上游文本都必须连同原始许可证/NOTICE 一起留在研究缓存或受控 quarantine，保留 source URL、revision、文件路径、原始 hash、修改说明和 removal plan。进入产品树时优先重写；若未来必须 vendoring，需另立 accepted receipt 并同步 `THIRD_PARTY_LICENSES.md`、SBOM 和最终包检查。

## 5. 给 Luna 的执行目标

```text
在不改变当前唯一 in_progress FGC-MCP010F 的前提下，按 LUNA_GITHUB_REPLICATION_PLAYBOOK.md 对指定上游仓库执行受控研究。每次只处理一个项目和一个明确模块：固定完整 commit，先写 research-authorized receipt，再从 GitHub 取得精确源文件到隔离缓存，进行许可证/依赖/动态代码/网络/文件系统审查。把学习结果重写为 ForgeCAD 自有 Schema、Rust Worker 或只读 MCP/Viewer contract；不得运行上游安装脚本、Python、Blender addon、socket server、模型权重或自动下载。未取得 approval: accepted、SBOM、恶意输入、确定性、资源和平台 Gate 前，不得修改 lockfile、安装包、active Skill 或 Runtime allowlist。
```

## 6. 失败与清理

- revision、许可证、文件 hash 或上游权限不明确：标为 `rejected`，删除缓存副本并保留原因；
- 静态审查发现动态执行、未授权网络/文件系统或无法界定依赖：只保留最小审计摘要，不保留可执行副本；
- benchmark 不稳定、资源超限或 lineage 不完整：不推广，保留 `approval: rejected` 和 fallback；
- 上游不再可用：现有 receipt 继续引用固定 revision；不得悄悄切换到新分支或新 release。

研究缓存和 quarantine 不是产品模块、Skill 或证据 PASS。任何删除都遵守 `DEPRECATED_ISOLATION_PLAN.md`，不得在脏 worktree 直接删除未知内容。
