# ForgeCAD 路线图与实施计划

状态：R0 已完成，R1 通用基础设施解耦进行中。

本文把“武神 Forge → AI CAD / 3D 打印 DFM Agent”拆成可独立评审、可回滚、带退出门的工程阶段。目标不是保留旧 Weapon API 的长期兼容，而是复用通用基础设施，建立新的 CAD/DFM 领域内核。

## 1. 计划基线

截至 2026-07-10，仓库真实状态是：

- 已有 Tauri、React、FastAPI、SQLite、内容寻址资产、Job/Step/Event/SSE、幂等、恢复与版本记录；
- `asset_store.py` 已从约 5210 行降至约 5116 行，SQLite connection、migration、内容寻址存储及首批 Repository/UoW 已提取，但业务工作流和大量 SQL 仍待拆分；
- `App.tsx` 已从约 865 行降至约 697 行，AppShell、Hash route、RuntimeProvider、JobEventProvider 与 SelectionProvider 已提取，旧任务恢复和页面业务组合仍待继续收敛；
- `main.py` 已从约 458 行降至约 54 行，legacy asset/job/system/weapon routes、错误映射与 base app factory 已分模块；
- 领域合同、表、API、UI 和发布门仍围绕 Weapon、Creative Recast、神经 3D 与 Unity；
- build123d、OpenCascade CAD Runtime、DesignSpec、FeatureGraph、STEP/3MF、DFM 和 Print Doctor 尚未实现。

旧代码是迁移输入，不是新产品完成度。

### 当前执行证据

| 项目 | 状态 | 证据 |
| --- | --- | --- |
| R0 tag / branch | 完成 | `legacy-wushen-v0.1`、`codex/refactor-cad-dfm-agent` |
| R0 ADR | 完成 | `docs/ADR/0001`–`0005` |
| R0 门禁快照 | 完成 | `docs/evidence/R0_BASELINE.md` |
| R1 SQLite / migration / object store | 完成首个切片 | `apps/agent/forgecad_agent/infrastructure` |
| R1 Repository / UoW | 部分完成 | Idempotency、Asset、Job exists、Checkpoint 已提取 |
| R1 API factory | 完成当前切片 | CORS/settings/app factory 与四组 legacy route modules 已提取 |
| R1 frontend shell | 完成当前切片 | AppShell、routing、Runtime/JobEvent/Selection Providers 已提取 |

R1 当前证据见 `docs/evidence/R1_FOUNDATION.md`。

## 2. 执行硬规则

1. 先冻结旧基线，再进行领域替换。
2. 先建立模块边界，禁止继续扩张 `asset_store.py` 和 `App.tsx`。
3. 不把 `WeaponDesignSpec`、`CreativeWeaponGraph` 或 `SkillGraph` 机械改名为 CAD 类型。
4. LLM 只能产生 Schema-valid 的结构化对象，不执行任意 Python。
5. build123d + OpenCascade 是 MVP 唯一权威 CAD 内核。
6. B-Rep 和 FeatureGraph 是工程源；GLB/STL 不是反向重建源。
7. 每个阶段以证据和质量门退出，不以“页面看起来完成”退出。
8. 每个副作用 API 保留幂等语义；每个耗时动作进入 Job。
9. 旧数据只读导入，不长期双写新旧领域。
10. 首个产品里程碑是 L 型支架完整纵向切片，不是品牌换肤或新首页。

## 3. 里程碑总览

| 阶段 | 目标 | 关键产物 | 参考周期 | 退出门 |
| --- | --- | --- | ---: | --- |
| R0 | 冻结与决策 | tag、分支、ADR、旧门禁快照 | 2–3 天 | 旧基线可恢复 |
| R1 | 通用基础设施解耦 | Repository、UoW、App Factory、前端壳 | 2 周 | 旧 smoke 不回归 |
| R2 | 新合同与新数据 | DesignSpec、FeatureGraph、migration、`/api/v1` | 2 周 | 合同/数据库门通过 |
| R3 | L 型支架 CAD 纵向切片 | build123d、B-Rep、STEP/3MF/STL/GLB | 3 周 | 几何与 STEP 回读门通过 |
| R4 | CAD 查看器与工作台 | selection、measurement、feature tree、version diff | 2–3 周 | 桌面 CAD E2E 通过 |
| R5 | DFM 与 Print Doctor | profiles、trimesh、findings、slicer adapter | 2–3 周 | DFM 真值集通过 |
| R6 | 结构化 AI 与 Beta | clarification、ChangeSet、五类模板、清理旧域 | 2–3 周 | Beta 发布门通过 |

参考总周期为 14–17 周，按 3–5 名核心工程人员估算。实际推进以质量门为准。

## 4. 阶段细化

### R0：冻结旧版本与架构决策

工作项：

- 给当前代码创建 `legacy-wushen-v0.1` tag；
- 创建 `codex/refactor-cad-dfm-agent` 或团队约定的重构分支；
- 保存 `m6:gate` 和现有 release gate 的输出；
- 新建 ADR：产品转向、CAD 内核、FeatureGraph 安全、第三方许可证；
- 冻结旧 Weapon Schema 和 `/api/weapons` 新功能；
- 明确旧数据库只读 importer 策略；
- 记录 `WUSHEN_* → FORGECAD_*` 的一版兼容读取计划。

退出条件：

- 能从 tag 恢复旧桌面与 Agent；
- README、设计、计划和操作文档采用一致的新边界；
- 所有团队成员理解旧 release gate 不是新产品发布门。

### R1：提取通用基础设施

后端：

- 从 `asset_store.py` 提取 connection、migration runner、content-addressed store；
- 提取 Job、Asset、Idempotency、Checkpoint repository；
- 建立 Unit of Work 和事务边界；
- 将 `main.py` 改为 app factory + route modules；
- API DTO、领域对象和数据库记录分离；
- Provider 通过 Port 注入，Repository 不创建 Provider。

前端：

- 将 `App.tsx` 缩为应用组合层；
- 提取 RuntimeProvider、JobEventProvider、SelectionProvider；
- 建立 router 与 AppShell；
- 明确 URL、server state、viewport state 与表单 local state 的归属。

退出条件：

- `asset_store.py` 不再承担完整业务工作流；
- route handler 不写 SQL、不组装文件、不直接调用 Provider；
- `App.tsx` 不再管理全部业务状态；
- 当前 `m6:gate` 仍通过，旧基线没有功能回归。

### R2：新领域合同与数据库

新增合同：

- `DesignSpec@1`；
- `FeatureGraph@1`；
- `BuildManifest@1`；
- `DfmReport@1`；
- `ChangeSet@1`；
- 通用 `JobEvent@2`。

新增表：

- `designs`、`design_versions`、`design_specs`；
- `requirement_sessions`、`clarifications`、`change_sets`；
- `feature_graphs`、`cad_builds`、`geometry_artifacts`；
- `printer_profiles`、`material_profiles`、`process_profiles`；
- `dfm_runs`、`dfm_findings`、`mesh_inspections`。

新增 API：

- `/api/v1/designs`；
- `/api/v1/designs/{id}/clarifications`；
- `/api/v1/designs/{id}/versions`；
- 通用 `/api/v1/jobs` 与 `/api/v1/assets`。

退出条件：

- 新安装迁移和重复迁移通过；
- 可创建中立 Design 并追加 Version；
- Python、TypeScript、OpenAPI 生成物无漂移；
- 新旧领域物理分目录，禁止新表引用 Weapon 领域主键。

### R3：第一个 CAD 纵向切片

只实现 L 型安装支架模板。支持：

- 底板长、宽、厚；
- 立板高度与厚度；
- 四孔孔径和两个方向孔距；
- 圆角与倒角；
- 两个三角加强筋；
- 打印方向和最大包围盒。

链路：

```text
DesignSpec
→ FeatureGraph
→ FeatureGraphCompiler
→ build123d / OpenCascade
→ B-Rep validation
→ critical dimension measurement
→ STEP / 3MF / STL / GLB
→ BuildManifest
→ immutable design version
```

实现约束：

- CAD Runtime 独立进程运行；
- 默认无网络，有 CPU、内存、时间、特征数和输出大小限制；
- 不接受 Python 代码，只接受通过 JSON Schema 的 FeatureGraph；
- 生产 STEP 必须重新导入并复测关键尺寸；
- 3MF 使用 lib3mf 正式写入与回读，不能只把 STL 改扩展名。

退出条件：

- 至少 100 组参数组合测试；
- 支持范围内有效实体率 ≥95%；
- 关键尺寸自动核验率 100%；
- 生产 STEP 回读率 100%；
- 所有失败返回 feature id、错误码与结构化诊断。

### R4：CAD 查看器与桌面工作台

新增：

- `NewDesignWizard`；
- `CadViewport` 与 three-cad-viewer adapter；
- three-mesh-bvh 选择和空间查询；
- 正交/透视相机、标准六视图、毫米网格；
- 面/边/特征选择、测量、截面；
- Feature Tree、参数、接口、约束；
- Build Volume 和 DFM overlay；
- ChangeSet 与版本透明叠加对比。

退出条件：

- 点击三角面能映射到 B-Rep face、Feature 和语义标签；
- 单 Feature 更新不重建整个 Viewer；
- 连续加载模型无明显 GPU 资源泄漏；
- 创建、构建、测量、修改、对比、重启恢复的桌面 E2E 通过。

### R5：FDM DFM 与 Print Doctor

实现：

- PrinterProfile、MaterialProfile、ProcessProfile；
- 几何确定性规则：空 Shape、多实体、开放壳、自相交、零厚度、小边/面；
- FDM 规则：成型空间、壁厚、孔径、间隙、悬垂、桥接、底面稳定性；
- 机械启发式：锐角应力集中、细长悬臂、层间方向风险；
- trimesh 网格加载、组件、水密性、法线、体积和包围盒检查；
- Manifold 只处理已分类且适合的 manifold 网格；
- PrusaSlicer 通过外部 CLI Adapter 提供可选估算。

退出条件：

- 每个 Finding 有规则版本、Profile 版本、实测值、位置、严重度和建议；
- curated blocker 真值集漏报为 0；
- 原始上传文件永不被修复结果覆盖；
- 未安装切片器时明确降级，不阻断基础 DFM。

### R6：结构化 AI、模板扩展与 Beta

实现：

- Instructor + Pydantic 的 RequirementInterpretation；
- 每轮最多 3 个真正阻塞的问题；
- TemplateSelector、ConstraintExtractor、RiskClassifier；
- 自然语言修改先生成 ChangeSet，再确认、重建和重跑 DFM；
- locked interface 保护；
- 自动修复最多 2–3 轮，超过后交回结构化诊断；
- 增加电子外壳、转接件、安装板、固定件/收纳件和简单夹具；
- 冻结旧 API，完成 importer，删除 Weapon/Skill/Unity 生产路径。

退出条件：

- 关键尺寸缺失时禁止生产导出；
- locked interface 修改保持率 ≥95%；
- 所有修改创建子版本，父版本不可变；
- 五类模板的支持范围、失败边界和 DFM 证据明确；
- 新桌面安装包可脱离源码目录运行。

## 5. 推荐 PR 顺序

每个 PR 只解决一个可验证边界：

1. **冻结与 ADR**：tag、产品转向、内核、安全、许可证决策。
2. **Storage / Repository**：内容寻址存储、Job/Asset repository、UoW。
3. **API / Models / App Shell**：app factory、routes、DTO、Providers、前端壳。
4. **新合同与数据库**：DesignSpec、FeatureGraph、migration、generated types。
5. **隔离 CAD Runtime**：build123d backend、compiler、sandbox、B-Rep validator。
6. **L 型支架纵向切片**：STEP/3MF/STL/GLB、尺寸与 round-trip gate。
7. **CAD Viewer**：three-cad-viewer、BVH、拓扑映射、测量、截面。
8. **DFM / Print Doctor**：profiles、trimesh、findings、truth set。
9. **结构化 AI 修改闭环**：clarification、ChangeSet、locked interface。
10. **Beta 清理与发布**：其余模板、importer、旧域删除、打包和 SBOM。

禁止将 2–10 合并成一个“大重构 PR”。

## 6. 新质量门

| Gate | 内容 | 阻断条件 |
| --- | --- | --- |
| C01 Contracts | JSON Schema、Python/TS、OpenAPI、旧字段扫描 | 任一漂移或出现 Weapon 字段 |
| C02 Database | fresh/repeat migration、FK、WAL、DAG | 迁移失败、孤儿记录、环 |
| C03 Templates | 100+ 参数组合、范围错误 | 崩溃、错误无诊断 |
| C04 Geometry | solid、bbox、体积、面积、孔位 | 关键几何超差 |
| C05 STEP Round-trip | 导出、回读、实体与尺寸复测 | 任一生产 STEP 无法回读 |
| C06 ChangeSet | 修改、接口保护、父版本不可变 | 锁定接口被静默破坏 |
| C07 DFM Truth Set | 薄壁、非流形、平台、孔、悬垂 | blocker 漏报 |
| C08 Jobs | 幂等、取消、超时、恢复、SSE replay | 丢事件或重复副作用 |
| C09 Sandbox | 文件、网络、shell、资源限制 | 任意越权或无限资源 |
| C10 Desktop E2E | 创建到导出、重启恢复 | 主链路失败 |

现有 `m6:gate` 只用于证明迁移前基线没有回归；现有 `release:gate` 仍包含 Unity 和旧安全文案，不能作为 ForgeCAD 发布门。

## 7. Definition of Done

一个功能只有同时满足以下条件才算完成：

- 合同、迁移、API、UI 和错误码一致；
- 单元、集成、round-trip 或 E2E 证据与风险相匹配；
- Job 可取消、超时、重试或明确声明不适用；
- 失败有结构化诊断，不能只返回自由文本；
- 工件能追溯输入哈希、合同版本、内核版本和 Profile 版本；
- 操作手册包含真实可执行步骤和恢复路径；
- 第三方依赖的版本、许可证与分发方式已记录。

## 8. 最近十个可执行动作

按顺序执行：

1. 创建旧基线 tag 并保存门禁输出。
2. 新建产品转向与 CAD 内核 ADR。
3. 建立 `forgecad_agent` 新包边界，不移动旧域代码。
4. 提取 SQLite connection、migration runner 和 content-addressed store。
5. 提取 Job、Asset、Idempotency repository 与 UoW。
6. 将 FastAPI 拆成 app factory 与 route modules。
7. 将 `App.tsx` 拆为 AppShell、router 与 Providers。
8. 新增五个核心 Schema 和类型生成管线。
9. 新增 Design/Version/CAD Build migrations 与 `/api/v1/designs`。
10. 实现隔离 build123d Runtime 的 L 型支架最小构建，不接 LLM。

第 10 步通过后，再开始 three-cad-viewer、DFM 和 AI 修改闭环。
