# Codex / Luna `GeometryProgram@2` 操作工作流

版本：2026-08-09
状态：`FGC-MCP010B structural source Gate PASS；Darwin OS memory hard cap deferred/NOT_RUN`；本文件是 V2 调用和审计指引，不代表 C 的视觉质量或 360°通过

当前 B 源码 reconciliation 已通过 source-focused Gate：B subtotal 为 52 contracts（44 历史 + 8 MCP010B，新增 `GeometryQualityReport@2`、`GeometryCandidateEvidence@1`）、MCP006 Skill integrity、isolated Worker/raw V2、V2 restore hardening 与 closed GLB profile。当前全仓源合同为 115；本文件只描述 B authoring/readback，Agentic durable session/checkpoint/RepairIntent 另见 `docs/ADR/0026-agentic-design-runtime.md`。3c/f488/bfa56/d9 Dev.app/CLI evidence 是历史或结构 cohort；本工作流不宣称 PBR、reference similarity、human review 或 360°。

## 1. 用途和边界

本文件让 Codex 或 Luna 在 MCP010B 期间以同一条受限路径发现 Runtime 实际可执行的几何 Operator，构造 `GeometryProgram@2`，并只根据 Runtime 对 GLB JSON/BIN/accessor 的回读决定是否停止。

它只覆盖结构真值：封闭参数、单位、预算、Part/source/material lineage、网格拓扑、UV/tangent 数值和 GLB 自包含性。它不证明参考图相似度、渲染质量、材质保真度、人体评分或完整 360°。当前单张三分之四参考也不能解除 `HQ_360_PASS=BLOCKED_REFERENCE_COVERAGE`。

## 0.1 最近一次真实单图演练

2026-08-10 已用用户授权的单张机器人 PNG 在当前 `d9c23b…ac0bd` package/isolated Codex CLI cohort 真实运行：`project_create → inline reference_import → skill_list → operator_catalog_get → geometry_program_hash → geometry_prepare → candidate_get/artifact_readback_get/job_get → quality_get`。脱敏证据见 [`docs/evidence/mcp010b/real-reference-robot-structural-run.json`](../docs/evidence/mcp010b/real-reference-robot-structural-run.json)。

该运行产生了 `GeometryProgram@2` 的 23 个语义 Part、9,964 triangles、1,592,884-byte 自包含 GLB；`ArtifactReadback@2` 的 invalid index、non-finite、退化面、boundary/non-manifold、winding、UV、tangent、external URI 均为 0，Part/source/material coverage 均为 1.0。`program_sha256`、catalog digest、candidate artifact 和参考 object hash 均已记录并绑定。它不是高质量视觉结果：当前 `QualityReport@1` 的 limited aspect proxy 为 `0.546637724065619 < 0.55`，像素 silhouette、landmark、region、PBR texture、human review 和 360°仍未运行；candidate 保持未确认、未创建 version/export。

同一参考随后又运行了一个 51-node primitive detail blockout（包含 visor、panel、emitter、cable、joint、limb 和 foot Part）。它生成 16,496 triangles、2,658,940-byte GLB，严格 BIN/accessor readback 仍为零错误、coverage 为 1.0，但 limited aspect proxy 降为 `0.4604316359607936 < 0.55`。这不是失败的结构编译，而是一个重要的质量实验结论：primitive 数量和 triangle 数量不能代替 camera/render/mask 的视觉比较；该 candidate 同样未确认、未创建 version/export。证据见 `docs/evidence/mcp010b/real-reference-robot-detail-blockout.json`。

Codex 同时整理了一份脱敏 `SubjectProfile@1` intake note（[real-reference-subject-profile.json](evidence/mcp010b/real-reference-subject-profile.json)），包含可见区域、近似 normalized landmarks、confidence 以及 rear/far-side/feet 的 `inferred/unknown` 标记。当前 `reference-intake` Skill 的 Runtime operators 尚未 active，因此这份文件只能帮助下一阶段复用参考理解，不能替代 Runtime producer、silhouette/landmark/region metrics 或人工评审。

本轮不接入外部脚本、BlenderMCP、FreeCAD MCP、xatlas、glTF Validator、远程 image-to-3D 或 AssetPack。Manifold 已按 MCP010D adoption receipt 以固定 C API 源码编入隔离 Geometry Worker，当前提供同一 Part 的 bounded union/difference/intersection；其余项目仍按 MCP010C–E/MCP012 的受控采用流程。

MCP010B Dev.app cohort `3c6f59f…7140`（pre-graph）和 `f4885b11…6bc1`（graph）均为历史 package receipt，必须保留，不能冒充当前验证。当前 `bfa56ac…de9` 是新建的 52-contract Dev.app cohort：它的授权参考 CLI structural receipt生成未确认的 12 Part/896 triangle/161104-byte candidate（`chest-shell` 按顺序输入 chest-shell/chest-panel），并有 matching packaged MCP/Runtime/Worker cohort。用户完整重启后的 live Desktop 已证明 32 工具、Ready、cohort match、catalog/hash 与项目只读回读；当前 d9 隔离 Codex CLI 又完成了同一七调用 V2 reference→hash→prepare→readback 结构链，证据见 `docs/evidence/mcp010b/dev-app-primitive-knowledge-codex-cli-v2-current-repeat.json`。无论新旧开发 package 都不证明视觉质量、PBR V2、360°或 Darwin 512 MiB OS 总内存硬上限。

## 2. 先发现真实能力，不能猜 Operator

在任何几何写入前，Codex 必须按下面顺序读取 Runtime：

```text
capabilities_get
→ runtime_status（Runtime 必须为 Ready）
→ operator_catalog_get
```

`operator_catalog_get` 是 Codex 可调用的 Runtime-owned 只读表面；它返回值必须与 `forgecad://operators/catalog` resource、`capabilities_get.operator_catalog_sha256`、V2 artifact/readback digest 完全一致，不能成为第二套 catalog 真值。控制端也可在 raw/resource 验收时读取该 URI 作交叉比对。工具读取失败、Runtime 未 Ready、resource 不存在或返回内容不是 `OperatorCatalog@1` 时，停止 V2 写入并记录 `RUNTIME_UNAVAILABLE`、`CAPABILITY_UNAVAILABLE` 或实际 typed error。不得从文档、Skill Bundle 或旧 receipt 猜测 catalog hash。

当前源码中的 MCP010B catalog 只声明一个 active Operator：

| 字段 | 当前允许值 |
|---|---|
| `operator_id` | `forgecad.geometry.primitive@2` |
| `input_arity` | `min=0, max=0` |
| `output_kind` | `triangle-mesh` |
| `part_output_required` | `true` |
| `supported_shapes` | `box`、`cylinder`、`ellipsoid`、`sphere` |

读取后必须同时检查：

1. `catalog.geometry_program_schema_version == "GeometryProgram@2"`；
2. catalog 的 `canonical_sha256` 为 64 位小写 SHA-256；
3. `capabilities_get.operator_catalog_sha256` 存在且与 catalog 的 `canonical_sha256` 相同；
4. program 中只能使用 catalog 中 `status="active"` 的 Operator 和其公布的形状。

当前 MCP010D catalog 的 node input arity 已支持真实有序 DAG：primitive leaf 与 transform/mirror/array、profile/loft/revolve/sweep、panel/vent/joint/part-output，以及 Boolean union/difference/intersection 的输入按 `inputs` 绑定；`part_outputs[].input_node_ids` 仍是语义 Part sink，允许一个 Part 聚合多个 detail source，并为每个 source 保留单独的 `source_node_id` 回读 binding。每个 node input 必须指向更早节点且不能被多个下游复用；每个 source 必须在下游或最终 sink 中正好消费一次；空、未知、重复、循环和 unconsumed input 都必须 fail closed。Boolean 必须恰好两个输入，只支持同一 Part scope 的 `union`/`difference`/`intersection`；通用 mesh Boolean 仍不开放。

## 3. Skill 是建议来源，不是执行权威

可在设计阶段读取 `skill_list` / `skill_get`，但只能将它们用于 typed Recipe、限制和 benchmark 说明。Runtime 的 Skill manifest 现额外返回：

```text
execution_availability = active | partial | unavailable
missing_operator_ids = [...]
```

只有一个 Bundle 锁定的每个 Operator 都被当前 Runtime/Worker cohort 真实执行时，它才可显示 `active`。不能因为 Bundle 提到一个 Operator，或某个 V1 兼容解析器接受相近参数，就把 Skill 标为 active。对 V2 几何调用，live `operator_catalog_get`（并以同值 resource 作审计比对）始终优先于 Skill metadata。

## 4. 构造封闭的 `GeometryProgram@2`

先完成参考的可见区域清单和稳定语义命名；不可见的背面、腿脚细节或内部结构必须写为 `unknown`/`inferred` 的设计说明，而不是伪装成参考事实。随后构造下列**完整且没有额外字段、且故意省略 `canonical_sha256`**的 hash draft：

```json
{
  "schema_version": "GeometryProgram@2",
  "project_id": "<existing-project-id>",
  "representation_plan_sha256": "<existing-64-lowercase-hash>",
  "operator_catalog_sha256": "<value-read-from-live-catalog>",
  "units": {
    "length": "meter",
    "angle": "radian",
    "coordinate_system": "right-handed-y-up"
  },
  "budgets": {
    "max_nodes": 2,
    "max_triangles": 250000,
    "max_glb_bytes": 67108864,
    "max_worker_memory_bytes": 536870912,
    "max_runtime_ms": 10000
  },
  "nodes": [
    {
      "node_id": "chest_shell",
      "operator_id": "forgecad.geometry.primitive@2",
      "inputs": [],
      "parameters": {
        "shape": "box",
        "size_m": [1.2, 1.6, 0.55],
        "position_m": [0.0, 1.7, 0.0],
        "rotation_rad": [0.0, 0.0, 0.0]
      }
    },
    {
      "node_id": "chest_panel",
      "operator_id": "forgecad.geometry.primitive@2",
      "inputs": [],
      "parameters": {
        "shape": "box",
        "size_m": [0.72, 0.36, 0.08],
        "position_m": [0.0, 1.72, -0.32],
        "rotation_rad": [0.0, 0.0, 0.0]
      }
    }
  ],
  "part_outputs": [
    {
      "part_id": "chest_shell",
      "input_node_ids": ["chest_shell", "chest_panel"],
      "material_zone_id": "zone-white-shell",
      "solid": true
    }
  ]
}
```

预算可以比上限更低，不能更高。`max_worker_memory_bytes` 和 `max_runtime_ms` 是本阶段受限输入合同；固定同级 Worker 的 10 秒 kill/reap 与 accepted-result peak-RSS 后验拒绝已通过，但它们不等于 Darwin 512 MiB OS 总内存硬门或跨机器性能 Gate 已通过。

每个 node 必须恰好含有 `node_id`、`operator_id`、`inputs`、`parameters`：

```json
{
  "node_id": "chest_shell",
  "operator_id": "forgecad.geometry.primitive@2",
  "inputs": [],
  "parameters": {
    "shape": "box",
    "size_m": [1.2, 1.6, 0.55],
    "position_m": [0.0, 1.7, 0.0],
    "rotation_rad": [0.0, 0.0, 0.0]
  }
}
```

使用 catalog 当前允许的四种 shape 时，参数必须严格匹配对应 variant：

| shape | 必填尺寸字段 | 分段字段 |
|---|---|---|
| `box` | `size_m: [x,y,z]` | 无 |
| `cylinder` | `radius_m`、`height_m` | `radial_segments`（8–64） |
| `ellipsoid` | `radii_m: [x,y,z]` | `longitude_segments`（8–64）、`latitude_segments`（4–64） |
| `sphere` | `radius_m` | `longitude_segments`（8–64）、`latitude_segments`（4–64） |

每个 variant 同时必须有 `shape`、`position_m` 和 `rotation_rad`。长度使用米，角使用弧度；position 的每个坐标范围为 `[-10, 10]` m，box 的每个 `size_m` 与 cylinder 的 `height_m` 是 `(0, 10]` m，sphere/cylinder 的 `radius_m` 与 ellipsoid 的每个 `radii_m` 是 `(0, 5]` m，rotation 的每个分量范围为 `[-2π, 2π]`。不要混入 V1 的 `size`、`position`、`rotation_y`、`segments`、`part_id` 或 `material_zone_id` node 参数。

`part_outputs` 是语义 Part sink 和 mesh source 的唯一显式连接。每个 output 有一个非空、顺序稳定且不重复的 `input_node_ids`；所有 V2 node 在全部 outputs 中必须正好出现一次。`part_id` 不可重复，但多个 source 可以合法属于同一个 Part：

```json
{
  "part_id": "chest_shell",
  "input_node_ids": ["chest_shell", "chest_panel"],
  "material_zone_id": "zone-white-shell",
  "solid": true
}
```

在 MCP010B，`material_zone_id` 只提供可回读的 lineage 标签与默认 factor material；它不是 MCP010E 的纹理、AssetPack、clearcoat 或完整 PBR 结论。

### Canonical hash

`canonical_sha256` 必须由 Runtime/Worker 计算，不能由 Codex、Skill、普通 JSON stringify 或 fixture 脚本猜测。调用：

```text
geometry_program_hash({
  schema_version: "GeometryProgramHashRequest@1",
  geometry_program_draft: <上面的严格 V2 draft>
})
```

返回必须是 `GeometryProgramHashResult@1`，其中 `geometry_program_schema_version="GeometryProgram@2"`、`validation_status="passed"`，且 `operator_catalog_sha256` 与刚读取的 catalog 相同。把返回 `canonical_sha256` 填入 draft，才得到可传给 `geometry_prepare` 的完整 program。hash 工具拒绝 V1、unknown field、预填 hash、catalog mismatch 或非法参数，且不创建 candidate、Job、CAS object、event 或 version。此约束已由 raw stdio 与隔离 source-built real Codex CLI V2 structural receipt 验证；它仍不构成高质量、视觉或 360°结论。

## 5. Prepare 调用与响应检查

仅在 write tools 已按用户授权可见、项目存在、上一步 catalog/hash 校验成功后，调用。完整 V2 program 的 `project_id` 必须与下列 outer `project_id` 完全相同；不同时 Runtime 必须在编译、candidate/Job/CAS 持久化之前拒绝：

```text
geometry_prepare(
  project_id = <existing-project-id>,
  request = {
    typed: "geometry",
    geometry_program: <canonical GeometryProgram@2>
  }
)
```

不要为 V2 夹带 `AppearanceProgram@1` 或调用 `appearance_prepare` 期待 V2 纹理结果；当前 Runtime 对 V2 appearance 路径应 fail closed，并说明 `AppearanceProgram@2`、atlas 和 PBR texture receipts 属 MCP010E。

成功响应只能按下面的关系判读：

```text
GeometryPrepareResult@2
  ├─ candidate        仍是待用户批准的候选，不是 immutable version
  ├─ job              本次受限编译记录
  ├─ operator_catalog 与调用前读取的 catalog hash 相同
  └─ artifact: ArtifactReadback@2
```

对 `ArtifactReadback@2`，Codex 必须检查：

1. `artifact_id == object_sha256`，`mime == "model/gltf-binary"`；
2. `program_sha256` 与刚提交 program 的 canonical hash 相同；
3. `operator_catalog_sha256` 与调用前读取的 live catalog 相同；
4. `validator_status == "passed"` **且** `hard_gate_passed == true`；
5. `integrity.glb_parse_status == "passed"`；
6. 下列计数全部为 0：`invalid_index_count`、`non_finite_count`、`degenerate_triangle_count`、`boundary_edge_count`、`non_manifold_edge_count`、`winding_error_count`、`uv_non_finite_count`、`zero_area_uv_triangle_count`、`tangent_non_finite_count`、`tangent_orthogonality_error_count`、`tangent_handedness_error_count`、`metadata_mismatch_count`、`external_uri_count`；
7. `part_coverage`、`source_coverage`、`material_zone_coverage` 全部严格等于 `1`；
8. `part_bindings` 每个 `input_node_id` 产生一项单源 `(part_id, source_node_id, material_zone_id, solid, triangle_count)` binding；多项可共享同一 `part_id`，但所有 source、顺序和总 triangle coverage 必须与所提交的 outputs 一致，且 `size_bytes`、node、triangle 和 GLB 预算均未超限；
9. 保留 `readback_config_sha256`、artifact hash 和 report hash，供同机五次重复的确定性 Gate 使用。

任意一点不满足时，停止链路、读取 `candidate_get` / `artifact_readback_get` 以保留 Runtime 真值、记录实际 error 或 counts，并且不调用 `candidate_confirm`。严格 readback 的通过只表示结构硬门通过；它不替代 MCP010C 的固定相机/九 AOV/参考指标，也不授权自动确认。

## 6. V1 过渡规则

`GeometryProgram@1`、`GeometryPrepareResult@1` 和 `ArtifactReadback@1` 仍服务 MCP007–009 已存在的 appearance / confirm / restore / CAS export 路径。它们不做历史对象迁移或改写，也不能被包装成 V2 结果。

| 情形 | 正确动作 |
|---|---|
| 需要重放已确认的 V1 version | 使用原 V1 lineage；不得升级其 schema/hash |
| 需要继续已验收的 V1 appearance golden path | 显式使用 V1，并把结果标为 legacy-compatible，而非 MCP010B V2 evidence |
| 想获得 V2 catalog/readback | 从 live catalog 重建新的 `GeometryProgram@2`，不能把 V1 字段强塞进 V2 |
| 想要 V2 texture/atlas/AssetPack | 停止；等待 MCP010E 的 `AppearanceProgram@2` 及其 adoption/receipt |

V1 的物理 GLB 回读可以帮助发现坏工件，但 V1 成功不可替代 `ArtifactReadback@2` 的 catalog binding、完整 source binding 或 MCP010B 证据。

## 7. MCP010B 的明确停止线

本阶段在严格回读之后停止。不得在 B 中：

- 把固定四 pass 或 `quality_get` 的 aspect-ratio limited 结果写成 silhouette、landmark、region 或人评通过；
- 新增或假称九 AOV、camera calibration、diff、visual review、human receipt 或 `PARTIAL_VISIBLE_VIEW_PASS`；
- 新增高细节 Operator、Boolean、Manifold、mirror/array 或双侧参数复制；
- 下载/安装材质、HDRI、纹理、xatlas、mikktspace、KTX2 或 glTF Validator；
- 使用 Viewer、Three.js、GLB 可打开、Skill availability 或 Codex 文字自评取代 JSON/BIN/accessor 回读；
- 在单视图下声明高质量 360°模型，或确认未通过 readback 的候选。

独立 geometry Worker 的 crash/timeout/FD isolation benchmark 与 accepted-result peak-RSS 后验门已有 MCP010B PASS receipt；Darwin 512 MiB OS 总内存硬门、参考图视觉基线、visual metrics 和人工评分仍没有 PASS receipt。真实 Codex V2 host E2E 的 structure-only attempt 1 receipt 为 `BLOCKED`、attempt 2 为 `PASS`；它们不会替代剩余 memory/visual/human Gate。所有状态必须在 `docs/evidence/mcp010b/manifest.json` 分别保持 `NOT_RUN`、`BLOCKED` 或实际运行后的状态，不能从历史 MCP007–009 receipt 推导。

## 8. Luna 交接清单

每次 MCP010B 运行都记录以下最小证据，不记录用户图片字节、prompt、本机绝对路径或 secret：

```text
Runtime cohort / capabilities hash
OperatorCatalog canonical_sha256
GeometryProgram@2 canonical_sha256
GeometryPrepareResult@2 candidate_id + job_id
ArtifactReadback@2 object_sha256 + readback_config_sha256
all integrity counts and coverage values
test command + exit code
PASS / FAIL / BLOCKED / NOT_RUN distinction
V1 transition used? yes/no, with reason
```

上述 contracts、negative fixture、worker、Runtime/MCP focused 和适用 aggregate Gate 已形成 B structural source Gate；Darwin OS memory hard cap仍 deferred/NOT_RUN，故 B 以 `blocked/deferred` 留账，不把受限预算写成硬上限。MCP010C 已由独立 Goal 领取；本文件不改变 C 的视觉质量或 360°能力状态。
