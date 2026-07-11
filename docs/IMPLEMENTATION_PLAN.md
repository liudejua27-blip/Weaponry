# ForgeCAD 路线图与实施计划

状态：R0 已完成；R1 当前退出边界已完成；产品主线已校准为 **Weapon Concept Pack first**。

本文保持 R0–R6 的可回滚阶段结构。P0 先完成模块化武器概念设计闭环；参数化 CAD/DFM 作为独立后续 Engineering Pack，不再阻塞第一阶段产品验证。

## 1. 计划基线

截至 2026-07-11：

- 已有 Tauri、React、FastAPI、SQLite、内容寻址资产、Job/Step/Event/SSE、幂等、恢复和追加式版本；
- `asset_store.py` 已从约 5210 行降至约 1449 行；connection、migration、object store、Repository/UoW、Job query/command/recovery、asset upload、library/version、Creative Recast、Create Weapon、Patch、Generate-3D、Worker Runtime 和 Unity Export 已提取；10 个 workflow facade 方法最长 20 行；
- `App.tsx` 已从约 706 行缩为 21 行组合根；AppShell、Hash route、Runtime/JobEvent/Selection Providers、旧工作台控制器/渲染、任务持久化、资产选择器和懒加载工作台已提取；
- `main.py` 约 97 行；legacy route groups、Concept services 和 app factory 已拆分，入口仍只负责应用组装与 worker 生命周期；
- `#/cad` 已按九区布局切换到“概念/组装/精修/检查/展示”，并接入真实 Project/Version/ModuleGraph、GLB、Connector 与 Concept 源包导出；
- 新 Concept 合同、Project/Profile/Version、Module/Connector registry、ModuleGraph、ChangeSet、QualityRun、Brief/Module/Change Planner Provider 边界、确定性规则降级、planner provenance、JobEvent@2/SSE、可追溯源包、combined GLB、OBJ/MTL、透明/爆炸 PNG、front/side/top、8 帧 turntable 与 MP4 已实现；Blender 4.2.22 已对工作台导出的 10 模块 reference、visual-v2 三模块和十模块 visual candidate combined GLB 完成真实 DCC 往返；真实模型 AI 质量指标、最终高质量资产和纹理交换尚未实现；
- `ModulePackManifest@1`、资产目录/许可证/GLB 结构校验、release 覆盖门和幂等批量导入已实现；Blender 4.2.22 LTS 已真实生成视觉层级增强的十模块 visual candidate `.blend`/GLB/thumbnail，完成只读 re-export、source hash 不变、完整 9 节点质量通过及 combined GLB DCC 往返（25808 顶点/10716 三角）。`FormalModuleReview@1` 仍如实拒绝 starter 许可证、未人工批准与低评分，因此正式资产仍未晋级；
- ChangeSet 审计批量导出已实现：当前服务端筛选形成确定性 JSONL、可选 CSV、逐文件 SHA-256 Manifest 和内容寻址 ZIP；`project_lifetime` 记录无单包删除 API，桌面可下载且 Agent 重启后回读。该策略不等于法规级 WORM、legal hold 或独立灾备；
- Library backup/restore CLI 已实现：SQLite Backup API 快照归一化为独立 `journal_mode=DELETE` 文件，只复制快照中 legacy/Concept asset 表真实引用的内容寻址对象，保存 schema/table/hash/size/capacity Manifest；恢复演练 CLI 可连续测量 backup/verify/restore/Agent 回读、吞吐和相对基线容量增长，并拒绝把已知 reference/smoke generator 申报为正式资产证据；`formal_blender_10_12` 还强制 `formal_release_10_12` 晋级报告与恢复 GLB hash 集合一致。10 模块参考库和十模块 Blender candidate 均已跑通多轮演练；candidate 报告明确为 `unclassified`，正式 Blender/代表性用户资产库仍待执行；
- 服务端 `weapon-concept-geometry/1.3` 已从版本绑定的 Spec、ModuleGraph 与内容寻址 GLB 计算 Mesh/Assembly Findings：除精确穿插、triangle provenance、Connector 对齐和保守表面间隙外，已覆盖重复面、内嵌封闭组件、组件间密度离群、项目总三角预算、P0 LOD0 合同和根中面对称占位偏差；桌面 Finding 会聚焦并高亮关联节点/局部 triangle。正式 Blender truth set、Tauri 大网格阈值与多 LOD 运行时尚未完成；
- build123d、OpenCascade、FeatureGraph、STEP/3MF 和 DFM 尚未实现，且不再属于 P0 主链。

旧代码是迁移输入；当前工作台是参考实现，不代表新领域完成。

### 当前证据

| 项目 | 状态 | 证据 |
| --- | --- | --- |
| R0 tag / branch | 完成 | `legacy-wushen-v0.1`、`codex/refactor-cad-dfm-agent` |
| R0 ADR / baseline | 完成 | `docs/ADR`、`docs/evidence/R0_BASELINE.md` |
| R1 infrastructure | 主要通用切片完成 | `forgecad_agent/infrastructure` |
| R1 application services | 后端 workflow 边界完成 | Job query/command/recovery、Asset、Library、Creative Recast、Create Weapon、Patch、Generate-3D、Worker Runtime、Unity Export services；facade 仅保留组合/adapter/helper |
| R1 API factory | 完成当前切片 | legacy routes + base app factory |
| R1 frontend composition | 当前退出边界完成 | 21 行 `App.tsx` 组合根；router、AppShell、Providers、controller、render、persistence、selectors；CAD/Preview3D 动态导入保留 |
| R1 workbench reference | 五阶段语义已完成 | `#/cad`、`design-qa.md`；真实 ModuleGraph/Connector 进入 R2–R3 |
| R2 concept contracts | 第一切片完成 | `packages/concept-spec`、`forgecad_agent.domain.concepts`、`r2:contracts-gate` |
| R2 project/version data + API | 第一切片完成 | migration `0009`、Concept repositories/service/routes、`r2:gate` |
| R2 module registry + graph | 第一切片完成 | immutable GLB registration、Connector compatibility、Graph persistence、restart smoke |
| R2 ChangeSet + child Version | 第一切片完成 | proposed/previewed/confirmed、protected node、stale base、parent immutability smoke |
| R2 QualityRun/Findings | 第一切片完成 | version-scoped report ingestion、finding persistence、idempotency、round-trip |
| R4 Brief/Module Planner | Provider 边界纵向切片完成 | deterministic rules、OpenAI-compatible strict JSON Schema、auto/strict failure semantics、migration 0014 provenance、registry recommendations、A/B/C structural variants、desktop selection preview、restart |
| R4 Change Planner | 可确认纵向切片完成 | `docs/evidence/R4_CHANGE_PLANNER.md`；migration 0015 actor/provider provenance、受限操作、registry/lock/path/no-op validation、JobEvent、ghost preview、reject/confirm、child Version、timeline 与 restart；真实 Provider 指标待测 |
| R4 Planner evaluation | 评测基础设施完成 | `docs/evidence/R4_PLANNER_EVALUATION.md`、`evaluations/r4/planner_truth_set.json`、20 Brief/20 Variant/20 Change/20 lock、hash、逐例结果、阈值、latency/token、deterministic baseline 全通过；当前未配置 live Provider，`real_provider_evidence_eligible=false` |
| R2 Concept JobEvent@2 | 同步主链完成 | Brief、Variant、Change Planner、Graph validate、QualityRun、Export jobs/events、cursor、SSE、restart |
| R2 Concept Export | 源包闭环完成 | `ConceptExportManifest@1`、ZIP、source GLB/spec/graph/quality、hash、artifact link、JobEvent、restart smoke |
| R3 workbench data binding | 四个纵向切片完成 | 米制 GLB→毫米视口、加载/选择/隐藏/聚焦/overlay、drag candidate、ChangeSet replace+snap、Undo/Redo、explode、restart |
| R3 Module Pack tooling | 完成 | `ForgeCADModuleNaming@1`、九类/8–12 release 门、UV/material/triangle/bounds/hash/license 校验、dry-run/import、idempotency/restart smoke |
| R3 reference assets | 完成可运行基线与 visual candidate | 10 GLB、九类、17 Connector、三材质/UV0/normal/thumbnail/license、9-node Graph、desktop E2E；十模块 Blender 4.2.22 visual candidate 已在隔离 Library 完成导入、完整 Graph、质量通过、导出、重启回读与 combined DCC 往返；最终 art 待人工制作 |
| R3 formal asset promotion | 合同与门禁完成、真实 starter 未批准 | `FormalModuleReview@1`、first_three/release_10_12、source/module Manifest/GLB/thumbnail/Pack+Module license hash、独立 reviewer、人工 checklist/评分、Blender generator、三角下限、最终许可证、基线 ID/Connector；真实 starter 验证返回许可证/人工审批/评分/core 三角下限阻断，无正式资产声明 |
| R3 Connector snap/mirror | 合成/API 与 Blender candidate 技术基线完成 | 100/100 含镜像数学样本、root/child 子树重定位、remap、mirror Version/Export、cycle conflict、lock、idempotency/restart；十模块 candidate 另完成 2/2 eligible front 替换、8/8 editable X 镜像、精确对齐、combined GLB/重启回读及锁定 root 拒绝。单节点镜像中 grip/top/side/armor 质量通过，front/rear/lower/storage 返回相交/包含 warning；连续八镜像压力分支共 8 warning。candidate 为 `unclassified`，正式资产指标待测 |
| R3 viewport lifecycle | 浏览器压力基线完成 | 20 轮 V3↔V4、1 canvas/1 active context、GC heap 与 renderer resource 上限；正式资产/Tauri 待测 |
| R3 operation timeline | 审计查询切片完成 | `updated_at + id` cursor、搜索、status/operation filter、rejected/stale diagnostic、桌面加载更多与 restart smoke |
| R3 ChangeSet audit archive | 批量导出切片完成 | migration `0016`、当前筛选 JSONL/CSV、hash Manifest、内容寻址 ZIP、`project_lifetime`、Job/artifact link、桌面下载与 restart smoke；WORM/legal hold/独立灾备未实现 |
| R3 Library backup/restore | 演练工具与参考基线完成 | `ForgeCADLibraryBackupManifest@1` + `ForgeCADLibraryRecoveryDrillReport@1`、SQLite Backup API、独立 DB、引用对象去重、多轮耗时/吞吐/容量增长、tamper/overwrite/secret/transient negatives、恢复后 Project/Version/全部 Module GLB/Planner Job/审计 ZIP 回读；formal 声明强制 promotion report hash link；正式 Blender 与代表性用户库待测 |
| R5 combined GLB | 第一切片完成 | static GLB merge、mm→m、Euler→quaternion、mirror scale、stable wrapper nodes、Manifest/hash、ZIP/direct download/restart、desktop E2E |
| R5 combined OBJ/MTL | 第一切片完成 | scene flatten、TRS/nonuniform scale/mirror、normal/winding、UV/material、meter units、Manifest/hash、ZIP/direct download/restart、desktop E2E |
| R5 deterministic PNG render | 第一切片完成 | 640×640 RGBA、auto-fit isometric、z-buffer、material color/light、preview/exploded、Manifest/hash、ZIP/direct download/restart、desktop E2E/visual QA |
| R5 multiview/turntable | 第一切片完成 | front/side/top、8 distinct frames、render-set ZIP、single Export reuse、API negatives/restart、desktop E2E/visual QA |
| R5 presentation delivery | 技术预览切片完成 | deterministic edge AA、soft contact shadow、FFmpeg MP4、Manifest/API/desktop download/restart；Blender 4.2.22 已完成 visual-v2 三模块、10 模块 reference 和十模块 visual candidate combined GLB 的真实往返；candidate OBJ/PNG/MP4 完整交付实测 16.4 s，最终批准资产全装配仍待执行 |
| R5 Mesh/Assembly quality | C07 规则覆盖切片完成 | immutable Spec/Graph/GLB、indices/degenerate/normal/UV/topology/bounds、duplicate/enclosed geometry、density outlier/triangle budget、P0 LOD0、root-plane symmetry、Connector alignment/gap、triangle BVH/SAT/containment/provenance、双节点/局部高亮、JobEvent/restart、desktop E2E |
| R6 packaging readiness | 门禁完整，真实二进制与流程验证待发布环境 | sidecar、`Cargo.lock`、bundle、CSP、capability、图标和文档入口均有校验；新门禁拒绝空、不可执行或无效平台头的 sidecar。当前占位 sidecar 被正确阻断，且当前机器无 Cargo/Rust |

## 2. 执行硬规则

1. 先冻结旧 baseline，再迁移领域。
2. 禁止继续扩张 `asset_store.py` 和 `App.tsx`。
3. 平台层保持通用，武器能力进入 `Weapon Concept Pack`。
4. P0 权威模型是 `WeaponConceptSpec + ModuleGraph + GLB modules`，不是 B-Rep。
5. 不把旧 WeaponDesignSpec、CreativeWeaponGraph 或 SkillGraph 机械改名。
6. 首版 AI 优先解析 Brief、选择模块、调整参数；不默认整模重生成。
7. 所有自然语言修改先形成结构化 `DesignChangeSet` 和幽灵预览。
8. Module、Connector、Version、Asset 必须使用稳定 ID；父版本和原始资产不可覆盖。
9. UI、报告和导出不得把概念 Mesh 声称为生产级 CAD 或制造就绪。
10. CAD/DFM Engineering Pack 使用独立合同、迁移、运行时和质量门，不能与 P0 长期双写。

## 3. 里程碑总览

| 阶段 | 目标 | 关键产物 | 退出门 |
| --- | --- | --- | --- |
| R0 | 冻结与决策 | tag、branch、ADR、baseline evidence | 旧版本可恢复 |
| R1 | 通用基础设施与产品校准 | Repository/UoW、app factory、frontend shell、双轨文档 | 旧 smoke 不回归，P0 边界一致 |
| R2 | Concept 合同与数据 | DomainProfile、WeaponConceptSpec、ModuleGraph、Connector、Version、ChangeSet | 合同/数据库门通过 |
| R3 | 模块系统与工作台 | 8–12 modules、选择/隐藏/替换/吸附/爆炸/保存 | 模块 E2E 与 GPU 生命周期通过 |
| R4 | AI Brief 与自然语言修改 | 三方案、模块推荐、ChangeSet、幽灵预览 | AI/锁定模块指标通过 |
| R5 | 检查、渲染与导出 | ModelQualityReport、GLB/OBJ/PNG/Manifest、爆炸图 | 检查与导出门通过 |
| R6 | Beta、资产扩展与发布 | 24–30 modules、用户测试、打包、旧域清理 | C01–C10 全部通过 |

CAD/DFM Engineering Pack 在 R6 首轮 Beta 证明产品价值后进入独立路线；其 DesignSpec、FeatureGraph、build123d、STEP/3MF 和 DFM 架构边界继续保留在 `DESIGN.md`，但不占用 P0 退出门。

## 4. 阶段细化

### R0：冻结旧版本与决策

已完成：

- 创建 `legacy-wushen-v0.1` 和重构分支；
- 保存旧门禁；
- 建立产品、内核、安全、许可证、数据迁移 ADR；
- 证明旧 baseline 可恢复。

新增范围修订必须使用 superseding ADR，不改写历史决策。

### R1：通用基础设施与产品校准

后端：

- 提取 connection、migration、content-addressed store；
- 提取 Job、Asset、Idempotency、Checkpoint repositories 和 UoW；
- 提取 legacy Job、Library、Asset Upload、Creative Recast、Create Weapon、Patch、Generate-3D、Worker Runtime 和 Unity Export services；
- `asset_store.py` 不再包含完整 workflow；共享资产/质量/事件 helper 作为注入端口保留，后续可继续下沉但不阻塞当前 R1 后端退出条件；
- 保持 route handler 不写 SQL、不组文件、不直接调用 Provider。

前端：

- AppShell、router、Runtime/JobEvent/Selection Providers；
- 将旧工作台业务控制器从 `App.tsx` 提出；
- 将 `#/cad` 更名和改造成 Weapon Concept Workbench；
- 五阶段：概念 / 组装 / 精修 / 检查 / 展示；
- URL、server state、viewport state、未提交表单状态归属明确。

退出条件：

- `asset_store.py` 不再承担完整业务工作流；
- `App.tsx` 只做应用组合；
- README、计划、设计、操作文档使用一致的 Concept-first 边界；
- `npm run r1:gate` 通过。

### R2：Concept 合同、数据库与 API

新增合同：

- `DesignDomainProfile@1`；
- `WeaponConceptSpec@1`；
- `ModuleGraph@1`；
- `ModuleAssetManifest@1`；
- `DesignChangeSet@1`；
- `ModelQualityReport@1`；
- 通用 `JobEvent@2`。
- `ConceptExportManifest@1`。

新增表：

```text
projects / project_versions / domain_profiles
module_assets / module_connectors / module_graphs
design_briefs / design_variants / design_change_sets
quality_runs / quality_findings / export_packages_v2
```

新增 API：

```http
POST   /api/v1/projects
GET    /api/v1/projects
GET    /api/v1/projects/{project_id}
POST   /api/v1/projects/{project_id}/versions
GET    /api/v1/module-assets
POST   /api/v1/module-graphs/{graph_id}/validate
POST   /api/v1/projects/{project_id}/variants
POST   /api/v1/versions/{version_id}/change-sets
POST   /api/v1/change-sets/{change_id}/confirm
POST   /api/v1/versions/{version_id}/quality-runs
POST   /api/v1/versions/{version_id}/exports
```

退出条件：

- fresh/repeat migration 通过；
- 可创建 `weapon_concept` Project 和追加 Version；
- Module/Connector 稳定 ID 与引用完整；
- Python、TypeScript、OpenAPI 生成物无漂移；
- 新表不引用旧 CreativeWeaponGraph/SkillGraph 主键。

### R3：模块系统与桌面工作台

首个固定项目：`寒地巡逻 S1`。

首批 8–12 个静态 GLB 模块覆盖：

```text
core_shell / front_shell / rear_shell / grip_shell
top_accessory / side_accessory / lower_structure
storage_visual / armor_panel
```

实现：

- 统一坐标、朝向、原点、比例、材质槽、UV、LOD 和缩略图；合同、CLI 与模板已落地，正式资产制作按 `MODULE_ASSET_GUIDE.md` 执行；
- 正式资产只有在 `FormalModuleReview@1` 锁定 source/export hash、独立审阅、全部人工项和评分通过后才可晋级；技术 Pack 通过或 synthetic smoke 不能替代人工最终资产；
- Connector overlay、拖放/替换、自动吸附、镜像、Transform、锁定；
- 模块树与视口同步选择、高亮、隐藏和聚焦；
- 爆炸视图、资源释放、Project 保存和恢复；
- 版本追加、Undo/Redo 和操作时间线。

退出条件：

- 能加载由 8–12 个模块构成的模型；
- 模块吸附和替换成功率 ≥95%；
- 更换主体后子模块可按 Connector 重新定位；
- 锁定模块不被普通操作改变；
- 连续加载/卸载无明显 GPU 泄漏；
- 重启后完整恢复 Project、Version 和 ModuleGraph。

### R4：AI Brief、方案与自然语言修改

工具：

```text
parse_design_brief
recommend_template
recommend_modules
create_variant
set_style_parameters
set_global_proportions
plan_change_set
```

工作流：

```text
Brief → Schema validation → A/B/C variants
→ 用户选择 → 指令 → ChangeSet → ghost preview
→ conflict/lock check → confirm → child version
```

AI 只能引用 registry 中存在的 Module/Connector ID。组件库无法满足需求时，局部生成进入单独 Job，原始资产不覆盖。

当前已完成 Brief Interpreter、Module Planner 与 Change Planner 边界：`deterministic_rules` 会解析紧凑/延展、明确长度/高度/握持角/细节百分比、颜色和对称等意图；`openai_compatible` 使用 strict JSON Schema，只能引用请求中提供的 node/module id。`auto` 模式外部 Provider 失败时显式降级并保存 attempted provider、错误、输入/输出 hash；`configured_provider` 失败直接返回错误。A/B/C 每个方案包含目标节点、结构 scale、注册模块建议和 rationale，并在服务端再次执行 lock/root/registry/Graph 校验。自然语言修改只允许 `replace_module`、`set_mirror`、`set_style`、`set_parameter`，先写入带 actor/provider/instruction/rationale/Job 的 proposed ChangeSet，再走既有 registry/lock/Connector/Graph preview；桌面显示半透明 ghost，用户明确确认才创建子版本，也可放弃并留下 rejected diagnostic。方案选择本身仍只切换 Planner 预览，不直接创建 Version。固定 80 阶段 truth set 与评测器已完成，deterministic baseline 四项为 100%，但真实模型质量评测尚未运行。

退出条件：

- Brief 解析成功率 ≥90%；
- 同一 Brief 返回三种明显不同方案；
- AI 修改成功率 ≥85%；
- 锁定模块保持率 ≥95%；
- 所有 AI 修改可解释、可撤销并创建子版本。

### R5：模型检查、渲染与导出

检查：

- 模块穿插、悬空模块、Connector 错位、非法缩放；
- 法线、非流形、对称差异、隐藏几何、网格密度、UV、LOD；
- Finding 点击后相机聚焦并高亮对应模块/区域。

已完成 C07 规则覆盖切片：索引、三角形数量、退化面、法线、UV0、开放/非流形边、清单 bounds、重复三角形、被另一封闭组件完全包裹的内部几何、单位表面积密度 8 倍中位数离群、Spec 总三角预算，以及当前 P0 只允许 canonical LOD0 的合同。装配层检查根节点局部 Z 中面的模块 AABB 占位配对、Connector `0.1 mm / 0.1°` 对齐和超过 `2 mm` 的保守表面间隙；未直连组件继续使用世界 AABB/BVH、triangle SAT、closed-mesh containment 和局部 provenance。Finding 点击会框选并高亮全部关联节点，叠加局部相交三角形。上述对称、密度和间隙都是概念资产代理指标，不是制造公差、结构或 DFM 结论。

展示与导出：

- 三分之四、正视、侧视、透明背景；
- 爆炸图、简单工作室灯光和转台动画；
- GLB、OBJ、PNG、Module Manifest 与 Project Report。

退出条件：

- 严重网格问题提示率 100%；
- GLB 导出成功率 ≥98%；
- 导出 Manifest 的 asset/version/hash 可追溯；
- 原始资产不被修复或导出覆盖。

### R6：Beta、资产扩展与发布

实现：

- 模块库扩展到 24–30 个高质量资产；
- 新手六步流程与专业参数渐进展开；
- 10–20 名目标用户完成固定任务；
- 统计完成时间、失败步骤、AI 修改失败和导出意愿；
- 清理旧 Weapon/Skill/Unity 生产入口，保留 importer 和 baseline tag；
- Tauri sidecar 打包、许可证、SBOM、干净机器安装/卸载。

退出条件：

- 新用户首个有效设计时间 <5 分钟；
- 首次 Project 完成率 ≥70%；
- Undo/Redo、版本回退、崩溃恢复正确率 100%；
- C01–C10 全部通过；
- 安装包可脱离源码运行。

## 5. 推荐 PR 顺序

1. **Concept-first ADR 与文档**：双轨产品、P0 输出和范围。
2. **R1 边界（已完成）**：Provider workflow、App controller、通用 services 与结构回归门。
3. **Concept contracts**：DomainProfile、Spec、ModuleGraph、ChangeSet、QualityReport。
4. **Concept database/API**：Project/Version/Module/Connector 与 `/api/v1`。
5. **Module fixtures**：第一套 8–12 个 GLB、Manifest、缩略图和许可证。
6. **Workbench IA**：五阶段、左 ContextPanel、右 Inspector、底部 Drawer。
7. **Module interaction**：选择、隐藏、替换、吸附、锁定、爆炸。
8. **Version/ChangeSet**：Undo/Redo、ghost preview、child version。
9. **Quality/Export**：检查定位、GLB/OBJ/PNG/Manifest。
10. **AI/Beta**：Brief、三方案、修改指标、打包和旧域清理。

每个 PR 必须有独立退出证据，禁止把 2–10 合成一个大重构。

## 6. C01–C10 新质量门

| Gate | 内容 | 阻断条件 |
| --- | --- | --- |
| C01 Contracts | Schema、Python/TS、OpenAPI、unknown field | 漂移、非法引用或不兼容 |
| C02 Database | fresh/repeat migration、FK、Version DAG | 失败、孤儿、环或父版本覆盖 |
| C03 Module Assets | 坐标、原点、比例、材质、UV、LOD、hash | 任一发布模块不合规范 |
| C04 Connectors | 类型、吸附、镜像、重定位、lock | 成功率 <95% 或锁定被破坏 |
| C05 Viewport | 选择、高亮、隐藏、爆炸、资源释放 | 错选、状态不同步或 GPU 泄漏 |
| C06 ChangeSet | before/after、ghost preview、Undo/Redo、Version | 变更不可解释或父版本被改写 |
| C07 Quality | 穿插、悬空、错位、法线、非流形、UV/LOD | 严重问题漏报 |
| C08 Jobs | 幂等、取消、超时、恢复、SSE replay | 重复副作用、丢事件或不可恢复 |
| C09 Export | GLB/OBJ/PNG/Manifest、hash、回读 | GLB 成功率 <98% 或工件不可追溯 |
| C10 Desktop E2E | Brief 到导出、重启恢复、打包 | 主链路或干净机器运行失败 |

现有 `m6:gate` 和 `release:gate` 只证明 legacy baseline；不得作为新产品发布证据。

CAD/DFM Engineering Pack 将另设 E01–E10：DesignSpec、FeatureGraph、B-Rep、STEP/3MF round-trip、DFM truth set 和 CAD sandbox，不与以上 P0 gate 混用。

## 7. Definition of Done

功能只有同时满足以下条件才完成：

- 合同、迁移、API、UI、错误码和领域 Profile 一致；
- 单元、集成、回读或 E2E 证据与风险匹配；
- Job 可取消、超时、重试或明确声明不适用；
- 失败有结构化 Finding/diagnostic；
- 工件可追溯输入、版本、资产 hash、Provider 和规则版本；
- 安全门覆盖 `secret/file-overreach`：密钥不得落入源码、日志或工件，对外 API 不暴露绝对路径，导入/导出不得越过 Library/Object Store 根目录；
- 原始资产、父版本和锁定模块不可被静默覆盖；
- 操作手册包含真实命令、备份和恢复路径；
- 依赖、模型资产和素材许可证已记录；
- 文档不把尚未实现的功能写成已完成。

## 8. 最近十个可执行动作

1. 由已完成视觉层级与 formal floor 的 core + 两个 front starter 进入人工 Blender 最终资产：保持 ID/Connector/Manifest 不变，由人工进一步处理轮廓、面板节奏、UV 与材质表现；换成最终许可证后，由非作者 reviewer 完成五项评分与批准，不能用 starter 或 synthetic smoke 代替。
2. 用正式资产测量 Connector 替换/镜像矩阵 ≥95%；显式镜像、自动吸附、root/child 子树重定位、拖拽候选、加载、选择、隐藏、聚焦、overlay、兼容替换、版本 Undo/Redo 与爆炸视图已完成合成/API/桌面基线。十模块 Blender candidate 已额外验证其仅有的 2/2 eligible front 替换与 8/8 editable 镜像、精确对齐、导出和重启回读；单节点镜像中 grip/top/side/armor 通过质量检查，front/rear/lower/storage 触发未连接组件相交/包含 warning，连续八镜像分支共 8 warning。因此不得把“操作通过”写作“组合可交付”。它是 `unclassified` 技术样本，不能计入正式 ≥95%。
3. 在正式 10–12 模块和代表性用户资产库上运行已完成的 `library:recovery-drill`，各至少 3 轮，保存备份/验证/恢复/Agent 回读耗时、吞吐、容量增长和未引用候选，再确定保留周期与 reference-aware GC；10 模块 reference fixture 已完成多轮稳定快照、全部 GLB hash 回读、基线增量和正式证据误报阻断，但不替代这两组真实报告。WORM/legal hold 不在当前承诺内。
4. 用正式 10–12 模块测量 PNG/MP4 时间与内存；starter core、工作台 visual-v2 三模块组合与 reference combined GLB 的真实 Blender round-trip 已通过，下一步是对正式 Blender 资产全装配重跑并评估纹理交换与 glTF Transform/Meshopt。
5. 将已完成的对称占位、隐藏几何、密度/预算和 P0 LOD0 规则迁移到正式 10–12 个 Blender 资产，测量误报/漏报、耗时和内存；多 LOD 只有在运行时切换与导出合同完成后再扩展。
6. 使用已完成的固定 truth set 和 live CLI，在明确授权的真实配置 Provider 上执行 80 次调用，采集 latency/token，并验证 Brief ≥90%、三方案差异度 100%、AI 修改成功率 ≥85% 和锁定保持率 ≥95%；先运行零网络、零费用的 `npm run agent:r4-evaluation-preflight`，只有本地配置就绪才由操作者承担 live 调用成本。当前 deterministic baseline 全通过但不具备真实 Provider 证据资格，当前环境严格返回 `EVAL_PROVIDER_NOT_CONFIGURED`。
7. 将 Concept jobs worker 化，补取消、重试、partial success 与 readiness。
8. AI 指标达标后扩展到 24–30 模块并执行首轮 Beta。
9. 用真实冻结 Agent 二进制替换当前占位 sidecar，并在含 Cargo/Rust 与平台签名权限的发布机上完成 compile、签名、安装/卸载和干净机验证。
10. 执行 C01–C10 发布审计并清理 legacy 生产入口。

第 6 步的真实 Provider 指标闭环通过后再执行第 7 步；AI 指标达标后才扩展第 8 步，第 9–10 步必须使用真实发布环境证据。CAD/DFM Engineering Pack 不提前占用 P0 主链。
