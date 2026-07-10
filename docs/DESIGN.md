# ForgeCAD 系统设计

版本：重构设计 v1（2026-07-10）
状态：目标架构已定义，实现尚处于迁移前基线。

## 1. 产品定义

ForgeCAD 是一个本地优先的 AI 参数化 CAD 与 FDM DFM 桌面 Agent。它面向单个、低风险、可 3D 打印的功能件，把不完整的用户意图转化为：

- 明确的需求、尺寸、单位、接口、假设和未知项；
- 受控、可审计、可重新构建的参数化特征图；
- 有效 B-Rep 与工程/打印交换格式；
- 可定位、带版本和实测值的 DFM 结论；
- 不覆盖父版本的自然语言修改记录。

### 1.1 当前系统与目标系统

当前实现：

```text
文本/草图
→ 结构解释与 Creative Recast
→ CreativeWeaponGraph / WeaponDesignSpec / SkillGraph
→ 概念图与 Patch
→ 神经 3D 粗模
→ Unity 导出
```

目标实现：

```text
文本/草图/参考图/尺寸
→ RequirementInterpretation
→ blocker 澄清与风险分类
→ DesignSpec
→ FeatureGraph
→ CAD Runtime / B-Rep
→ 几何与关键尺寸验证
→ DFM
→ ChangeSet / 新版本
→ STEP / 3MF / STL / GLB / 报告
```

这是领域内核替换，不是名词替换。

## 2. 范围和风险边界

首版支持 bracket、enclosure、adapter、mounting plate、simple fixture、holder/organizer 等单件 FDM 功能件。

首版拒绝或不支持：

- 武器及武器零部件；
- 安全关键、受监管或需要认证的用途；
- 医疗、航空飞行、汽车制动/转向、高压/承压、载人承重；
- 复杂装配、CNC、钣金、注塑、FEA、GD&T；
- 把神经网格声称为参数化 CAD；
- 把启发式 DFM 声称为结构安全认证。

风险策略：

| 风险 | 典型用途 | 系统行为 |
| --- | --- | --- |
| R0 | 装饰、无承载 | 正常生成和导出 |
| R1 | 普通功能件 | 正常生成，展示假设与限制 |
| R2 | 承载、热、电气、运动结构 | 警告、人工确认、限制自动结论 |
| R3 | 安全关键、受监管、受限用途 | 阻止生成或生产导出 |

## 3. 架构原则

### 3.1 LLM 不负责几何真值

LLM 可以：

- 提取用途、尺寸和接口；
- 识别缺失信息；
- 选择模板；
- 提议 DesignSpec、FeatureGraph 和 ChangeSet；
- 解释结构化构建或 DFM 错误。

LLM 不可以：

- 直接输出生产几何；
- 静默改动锁定尺寸；
- 生成并执行任意 Python；
- 绕过 Schema、Compiler、验证或风险策略。

### 3.2 单一权威几何链

```text
DesignSpec
→ FeatureGraph
→ build123d Compiler
→ OpenCascade B-Rep
→ validated STEP
→ 3MF / STL / GLB
```

MVP 不同时维护 build123d 和 CadQuery 两套权威实现。CadQuery 只保留在 `CadBackend` 接口的未来扩展位。

### 3.3 所有状态可追溯

每次构建、DFM、切片和导出都必须关联：

- design/version/build/job id；
- 输入合同与内容哈希；
- compiler、CAD kernel 与 runtime 版本；
- printer/material/ruleset 版本；
- 输出 asset id 与 SHA-256；
- 验证、警告和失败诊断。

### 3.4 本地优先和进程隔离

SQLite 与对象存储是本地权威数据。FastAPI 负责任务和应用用例；CAD 原生内核在独立进程运行，避免崩溃、超时或内存异常拖垮 Agent。

## 4. 目标系统结构

```text
┌───────────────────────────────────────────────────────┐
│ Tauri Desktop                                         │
│ New Design | CAD Workbench | Print Doctor | Jobs      │
│ Profiles | Library | Settings | Reports               │
└───────────────────────┬───────────────────────────────┘
                        │ HTTP + SSE
┌───────────────────────▼───────────────────────────────┐
│ FastAPI Local API                                     │
│ Routes → Use Cases → Workflow / Jobs                  │
├───────────────────────┬───────────────────────────────┤
│ Repository / UoW      │ External Ports                │
│ SQLite / Object Store │ LLM / Renderer / Slicer       │
└───────────────────────┴───────────────┬───────────────┘
                                        │ local IPC/HTTP
┌───────────────────────────────────────▼───────────────┐
│ Isolated CAD Runtime                                  │
│ FeatureGraph Compiler → build123d / OCCT → B-Rep      │
│ validation → tessellation → STEP/3MF/STL/GLB          │
└───────────────────────────────────────┬───────────────┘
                                        │
┌───────────────────────────────────────▼───────────────┐
│ DFM / Mesh / Slicing                                  │
│ Rules + Profiles | trimesh | lib3mf | Slicer Adapter  │
└───────────────────────────────────────────────────────┘
```

## 5. 模块边界

建议目标目录：

```text
apps/agent/forgecad_agent/
  api/                 # app factory、dependencies、thin routes、DTO
  application/         # commands、queries、use cases
  domain/              # design、geometry、manufacturing、dfm、jobs
  ports/               # CAD、LLM、slicer、renderer、repositories
  infrastructure/      # SQLite、object store、provider adapters
  runtime/             # worker、workflow、checkpoint、recovery

apps/cad-runtime/forgecad_runtime/
  compiler/            # FeatureGraph compiler、selectors、anchors
  kernel/              # build123d backend、OCCT validators
  exporters/           # STEP、3MF、STL、GLB
  sandbox/             # limits、workspace、process lifecycle

apps/desktop/src/
  app/
  features/new-design/
  features/design-workbench/
  features/viewport/
  features/feature-tree/
  features/dfm/
  features/print-doctor/
  features/versions/
  features/jobs/
  features/profiles/

packages/design-spec/
packages/cad-ir/
packages/dfm-rules/
packages/test-fixtures/
```

迁移期间允许 `SQLiteAssetStore` 作为 Facade，但其内部必须委托 Repository、UoW 和 Use Case；新 CAD 业务不得直接回填这个 5210 行类。

## 6. 核心领域模型

### 6.1 DesignSpec@1

DesignSpec 表达“要制造什么”和“哪些约束不可破坏”，不保存任意 CAD 代码。

```json
{
  "schema_version": "design-spec/1.0",
  "design_id": "des_01...",
  "name": "L 型传感器支架",
  "part_type": "bracket",
  "units": "mm",
  "purpose": "固定桌面传感器",
  "coordinate_system": { "up_axis": "Z", "front_axis": "Y" },
  "envelope": { "max_x": 256, "max_y": 256, "max_z": 256 },
  "interfaces": [
    {
      "id": "base_mount",
      "type": "bolt_pattern",
      "locked": true,
      "parameters": {
        "hole_diameter": 4.5,
        "spacing_x": 80,
        "spacing_y": 40
      }
    }
  ],
  "critical_dimensions": [
    { "id": "base_thickness", "value": 5, "tolerance": 0.2, "locked": false }
  ],
  "manufacturing": {
    "process": "fdm",
    "printer_profile_id": "printer_256_04",
    "material_profile_id": "petg_generic"
  },
  "assumptions": [],
  "unknowns": [],
  "risk_class": "R1"
}
```

不变量：

- 内部长度统一为 mm，边界负责单位转换；
- critical dimension 和 interface 有稳定 ID；
- locked 项不能被 AI 静默修改；
- blocker unknown 存在时禁止生产导出；
- 所有假设显式存入 `assumptions`。

### 6.2 FeatureGraph@1

FeatureGraph 是受限 CAD IR。首版允许：

```text
SketchRectangle / SketchCircle / SketchPolyline
Extrude / Revolve / Hole / Shell / Rib
Fillet / Chamfer
LinearPattern / CircularPattern / Mirror
Union / Difference
```

```json
{
  "schema_version": "feature-graph/1.0",
  "parameters": {
    "base_width": { "type": "length", "value": 100, "unit": "mm", "min": 40, "max": 300 },
    "base_thickness": { "type": "length", "value": 5, "unit": "mm", "min": 2, "max": 20 }
  },
  "nodes": [
    {
      "id": "base_sketch",
      "type": "SketchRectangle",
      "inputs": { "width": "$base_width", "height": 60 }
    },
    {
      "id": "base_extrude",
      "type": "Extrude",
      "depends_on": ["base_sketch"],
      "inputs": { "distance": "$base_thickness" },
      "semantic_tags": ["base", "top_mounting_surface"]
    }
  ]
}
```

Feature 不能使用不稳定的 `Face[7]` / `Edge[3]` 作为长期引用。选择器优先由 owner feature、几何类型、法向、相对位置、interface id 和 semantic tag 组成。

### 6.3 BuildManifest@1

每个构建记录：

```text
design_spec_hash
feature_graph_hash
compiler_version
build123d_version
occt_version
runtime_image_version
printer/material/ruleset versions
start/end/status
artifact ids and sha256
geometry metrics
validation results
```

### 6.4 ChangeSet@1

自然语言修改先形成可审计计划：

```json
{
  "instruction": "底板加厚到 7 mm，孔距不变，并增加两个加强筋",
  "parameter_changes": {
    "base_thickness": { "before": 5, "after": 7 }
  },
  "feature_changes": [
    { "operation": "add", "feature_type": "Rib", "count": 2 }
  ],
  "locked_interfaces_checked": ["base_mount"],
  "requires_confirmation": false
}
```

应用 ChangeSet 后必须创建子版本、重建、重测锁定接口并重跑 DFM。

### 6.5 DfmReport@1

Finding 的严重度固定为 `blocker | high | suggestion | info`。每条 Finding 必须包含：

- `rule_id` 与规则版本；
- 实测值和推荐范围；
- feature/face/region 位置；
- printer/material profile 版本；
- 是否支持自动修复及建议参数；
- 置信度；
- 说明它是确定性规则还是启发式风险提示。

## 7. 工作流与状态机

### 7.1 新设计

```text
ingest_input
→ classify_risk
→ classify_supported_part
→ parse_requirements
→ validate_units
→ detect_unknowns
→ request_clarifications
→ select_template
→ generate_design_spec
→ generate_feature_graph
→ compile_cad
→ validate_brep
→ verify_critical_dimensions
→ tessellate_preview
→ run_dfm
→ await_review
→ export
```

状态：

```text
draft → interpreting → needs_input → ready_to_build
→ building → validating → review_ready → exporting → exported
```

失败状态：

```text
build_failed / validation_failed / dfm_blocked
provider_failed / cancelled
```

### 7.2 澄清

每轮最多 3 个 blocker，只问影响几何、接口、制造或风险判断的问题。颜色、命名和非关键外观不能形成无限追问。

### 7.3 构建修复

```text
Compiler error
→ structured diagnostic
→ Agent proposes graph correction
→ schema and policy validation
→ rebuild
```

自动重试上限 2–3 次。超过上限返回失败特征、局部测量、建议参数和可选模板，不继续无限尝试。

## 8. API 设计

新 API 使用 `/api/v1`，所有耗时操作返回 `job_id`，所有副作用请求支持 `Idempotency-Key`。

```http
POST   /api/v1/designs
GET    /api/v1/designs
GET    /api/v1/designs/{design_id}
DELETE /api/v1/designs/{design_id}

GET    /api/v1/designs/{design_id}/clarifications
POST   /api/v1/designs/{design_id}/clarifications/{id}/answer

POST   /api/v1/designs/{design_id}/versions/{version_id}/builds
GET    /api/v1/builds/{build_id}

POST   /api/v1/designs/{design_id}/versions/{version_id}/change-plans
POST   /api/v1/change-plans/{change_id}/confirm
POST   /api/v1/change-plans/{change_id}/reject

POST   /api/v1/builds/{build_id}/dfm-runs
GET    /api/v1/dfm-runs/{dfm_run_id}

POST   /api/v1/mesh-inspections
GET    /api/v1/mesh-inspections/{inspection_id}
POST   /api/v1/mesh-inspections/{inspection_id}/repairs

POST   /api/v1/designs/{design_id}/versions/{version_id}/exports
GET    /api/v1/exports/{export_id}

GET    /api/v1/jobs
GET    /api/v1/jobs/{job_id}
GET    /api/v1/jobs/{job_id}/events
POST   /api/v1/jobs/{job_id}/cancel
POST   /api/v1/jobs/{job_id}/retry
GET    /api/v1/assets/{asset_id}
```

路由只做解析、调用 Use Case、响应转换和错误映射。SQL、文件组装、CAD 和 Provider 调用不能出现在 route handler。

## 9. 数据设计

保留并泛化：

```text
schema_migrations
idempotency_records
jobs / job_steps / job_events
provider_configs / provider_tasks / runtime_checkpoints
asset_files / export_packages
```

新增：

```text
designs / design_versions / design_specs
requirement_sessions / clarifications / change_sets
feature_graphs / cad_builds / geometry_artifacts
printer_profiles / material_profiles / process_profiles
dfm_runs / dfm_findings / mesh_inspections
```

关系：

```text
design
└─ design_version
   ├─ design_spec
   ├─ feature_graph
   ├─ reference_assets
   ├─ change_set
   ├─ cad_build
   │  ├─ STEP / 3MF / STL / GLB
   │  └─ build_manifest
   └─ dfm_run
      └─ dfm_findings
```

旧数据策略：

- tag 后的旧数据库只读；
- Weapon 记录最多导入为 legacy project/reference asset；
- CreativeWeaponGraph 和 SkillGraph 不转换为 FeatureGraph；
- 新版本只写新表，不长期双写；
- importer 是一次性、可重跑、带报告的工具。

## 10. CAD Runtime

`CadRuntimePort`：

```python
class CadRuntimePort(Protocol):
    async def build(self, request: CadBuildRequest) -> CadBuildResult:
        ...
```

Runtime 必须：

- 独立进程和临时工作目录；
- 默认禁用网络；
- 限制 CPU、内存、时间、特征数和输出大小；
- 只读 Schema-valid FeatureGraph；
- 不加载用户 Python；
- 记录 stdout、stderr 与结构化诊断；
- 超时或崩溃后由 Worker 回收；
- 生产 STEP/3MF 完成回读验证后才登记为 validated artifact。

查看器数据除 GLB 外还要携带 topology map，将 triangle/edge range 映射到 face id、feature id 和 semantic tag。

## 11. DFM、网格与切片

规则分三层：

1. 几何确定性：有效实体、开放壳、自相交、零厚度、小边/面、STEP 回读。
2. FDM 工艺：成型空间、壁厚、孔、间隙、悬垂、桥接、底面、空腔、方向。
3. 机械启发式：应力集中、细长悬臂、孔边材料、层间方向、加强筋风险。

第三层只输出“风险提示”，不输出认证结论。

工具职责：

- build123d/OCCT：权威 B-Rep、尺寸和拓扑验证；
- trimesh：上传网格的场景、组件、水密、法线、体积、包围盒；
- Manifold：适合输入上的网格布尔/规范化，不承诺修复所有坏 STL；
- lib3mf：3MF 元数据、写入与回读；
- PrusaSlicer Adapter：可选切片时间与耗材估算，缺失时可降级。

## 12. 前端设计

工作台布局：

```text
┌──────────────┬────────────────────────┬────────────────┐
│ 项目/需求/版本 │ CAD Viewport           │ 参数/接口/DFM   │
├──────────────┴────────────────────────┴────────────────┤
│ Agent 轨迹 / Job Steps / 错误 / ChangeSet 差异         │
└────────────────────────────────────────────────────────┘
```

状态归属：

- URL：当前 design/version/job；
- TanStack Query：designs、versions、jobs、profiles、reports；
- Zustand 或 reducer：相机、选择、截面、测量和 viewport tool；
- local state：未提交表单；
- 后端：权威项目、任务、版本和构建状态。

旧 Canvas 保留画笔、套索、缩放、图层和 undo/redo，但改为参考图比例标定、安装面、孔位、保留区、禁入区和尺寸锚点。参考图标注表达意图，不作为精确几何真值。

## 13. 第三方依赖与许可证

P0 直接依赖候选：

- build123d（Apache-2.0）；
- three-cad-viewer（MIT）；
- three-mesh-bvh（MIT）；
- trimesh（MIT）；
- Manifold（Apache-2.0，按需）；
- lib3mf（BSD-2-Clause）；
- Instructor（MIT）；
- CADGenBench（Apache-2.0，测试思路/依赖）。

隔离或专项审查：

- PrusaSlicer（AGPL-3.0）：外部 CLI Adapter，单独审查分发方式；
- NopSCADlib（GPL-3.0）：仅参考 BOM/爆炸图思想；
- Fusion 360 Gallery Dataset：非商业研究限制，不进入商业训练数据。

每个依赖 PR 必须固定 tag/commit、保存许可证、记录是否修改、声明链接/进程方式并更新 SBOM。此处不是法律意见。

## 14. 已冻结决策

- 临时代号 ForgeCAD，品牌名后置；
- 单零件 + FDM + 五类功能件先行；
- build123d + OpenCascade 单内核；
- 自有 DesignSpec + FeatureGraph，不执行任意代码；
- 3MF 是默认打印交付，STEP 是工程交换，STL 保持兼容；
- CAD Runtime 独立进程；
- Print Doctor 与设计 DFM 共用 Profile 和 Finding；
- 旧 Weapon/Skill/Unity 领域最终删除，不长期兼容；
- LangGraph、复杂装配、FEA、CNC、注塑和通用 CAD 编辑器后置。

## 15. 验证策略

验证层级：

```text
contract and migration tests
→ compiler unit tests
→ 100+ template parameter builds
→ geometry metrics regression
→ STEP/3MF round-trip
→ DFM truth set
→ ChangeSet and locked-interface tests
→ Job recovery and sandbox tests
→ desktop E2E
```

详细阶段、质量门和 PR 顺序见 [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md)，真实运行与故障处理见 [OPERATIONS.md](OPERATIONS.md)。
