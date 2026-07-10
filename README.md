# ForgeCAD（原武神 Forge）

ForgeCAD 是一个面向武器、武器零部件、精密机械结构与 3D 打印功能件的本地优先 AI CAD / DFM 桌面 Agent。它把自然语言、草图、参考图和真实尺寸转换为可编辑、可验证、可追溯的参数化设计，并输出 STEP、3MF、STL、GLB 与 DFM 报告。

> 当前仓库正在从“武神 Forge / 幻想武器美术资产”切换到 AI CAD / FDM DFM。桌面壳、任务系统、资产库和版本基础已经存在；CAD 内核、DesignSpec、FeatureGraph、DFM 与制造导出仍属于重构目标。本文不会把规划中的能力描述成已完成能力。

正式品牌名尚未冻结，文档暂用 **ForgeCAD**；代码包名和环境变量暂时仍保留 `wushen` / `WUSHEN_*`，直到迁移阶段完成。

## 产品目标

目标用户：

- 3D 打印服务商；
- 工业设计师与创客；
- 武器外形、结构与零部件设计人员；
- 小型硬件团队；
- 需要快速验证功能件的产品团队。

核心闭环：

```text
自然语言 / 草图 / 参考图 / 尺寸
→ 需求结构化与风险分类
→ 缺失尺寸澄清
→ DesignSpec
→ 受控 FeatureGraph
→ build123d / OpenCascade B-Rep
→ 几何与关键尺寸验证
→ FDM DFM
→ 自然语言修改与 ChangeSet
→ STEP / 3MF / STL / GLB / DFM 报告
```

产品价值不是“生成一个看起来像 3D 物体的文件”，而是：

> 把用户需求转换为可编辑、可验证、可稳定重建、可追溯交付的参数化功能件。

## 首版边界

首版聚焦单件或小型模块化组件的参数化 CAD/DFM，前期产品设计与回归以武器项目为主：

- 模块化武器外形、主体、枪管、握把、导轨、瞄具和接口类零部件；
- 支架与安装座；
- 电子设备外壳；
- 转接件；
- 安装板与简单夹具；
- 固定件与收纳件。

首版输入：

- 自然语言用途描述；
- 真实尺寸与单位；
- 草图或参考图片；
- 打印机、喷嘴、材料与制造目标。

首版输出：

- `DesignSpec@1`；
- `FeatureGraph@1`；
- 可重新构建的 B-Rep；
- STEP、3MF、STL 与 GLB 预览；
- `DfmReport@1`、参数表、版本差异与打印建议。

首版明确不做：

- 有机角色、动画、骨骼和游戏资产；
- Unity 导出；
- 复杂装配、完整 BOM、CNC、钣金、注塑；
- 自动 FEA、GD&T 或认证结论；
- 医疗、航空、汽车、高压、承压、载人承重等需要专业认证的自动结论。

首版允许并优先验证武器、武器零部件及相关精密机械结构。系统不得按“武器”类别一刀切拒绝；对膛压、热载荷、疲劳、材料和法定合规等尚未验证的条件，应明确标注工程假设与人工验证要求，而不是声称已经完成安全认证。

## 当前状态

| 能力 | 仓库现状 | 重构决策 |
| --- | --- | --- |
| Tauri + React 桌面壳 | 已实现 | 保留并重构应用壳 |
| FastAPI 本地 Agent | 已实现 | 保留，路由改为薄层 |
| SQLite、WAL、迁移 | 已实现 | 保留，新增 CAD 领域表 |
| 内容寻址资产存储、SHA-256 | 已实现 | 保留 |
| Job / Step / Event / SSE | 已实现 | 泛化并保留 |
| 幂等、取消、重试、恢复 | 已实现 | 保留 |
| R1 通用基础设施边界 | 已开始 | SQLite、migration、object store、首批 Repository/UoW、API factory、routing、RuntimeProvider 已提取 |
| 追加式版本记录 | 已实现，仍绑定 weapon | 泛化为 design version DAG |
| `CreativeWeaponGraph` / `SkillGraph` | 已实现 | 冻结并删除，不机械改名 |
| 图像生成与神经 3D | 已实现适配器 | 降级为可选概念参考 |
| Unity 导出 | 已实现 | 删除 |
| DesignSpec / FeatureGraph | 未实现 | 新建独立合同 |
| build123d / OpenCascade B-Rep | 未实现 | P0 权威 CAD 内核 |
| STEP / 3MF 工程导出 | 未实现 | 新建并做回读验证 |
| DFM / Print Doctor | 未实现 | 新建规则引擎与网格检查 |
| CAD 工作台 | R1 交互壳已实现 | 已有九区布局、武器参数、Three.js 视口、组件筛选、DFM/导出状态；真实 CAD Runtime 与持久化仍未接入 |

当前可运行主数据链仍是旧产品基线；`#/cad` 已提供新工作台交互壳，但它不等于 CAD 内核、真实 DFM 或制造导出已经完成。运行和验证方法见 [操作手册](docs/OPERATIONS.md)。

## 核心架构

```text
Tauri Desktop
  ├─ New Design / CAD Workbench / Print Doctor / Jobs / Profiles
  └─ HTTP + SSE
          ↓
FastAPI Local API
  ├─ Thin Routes
  ├─ Application Use Cases
  ├─ Repository / Unit of Work
  └─ Workflow / Jobs / Events
          ↓
Isolated CAD Runtime
  ├─ FeatureGraph Compiler
  ├─ build123d + OpenCascade
  ├─ B-Rep validation and measurement
  └─ STEP / 3MF / STL / GLB exporters
          ↓
DFM Engine
  ├─ geometry rules
  ├─ FDM process rules
  └─ printer / material profiles
```

三条不可破坏的原则：

1. LLM 只生成结构化需求、澄清问题和受控 FeatureGraph，不直接生成最终几何。
2. 不执行 LLM 生成的任意 Python、Shell、文件或网络操作。
3. `DesignSpec → FeatureGraph → B-Rep` 是工程源数据；STEP 是工程交换，3MF/STL/GLB 是派生产物。

## 技术选择

| 层 | P0 选择 | 用途 |
| --- | --- | --- |
| 桌面端 | Tauri + React + TypeScript | 本地工作台与 sidecar 生命周期 |
| 本地 API | FastAPI + Pydantic | DTO、用例、任务与事件 |
| 数据 | SQLite + 内容寻址对象存储 | 本地优先项目、版本与工件 |
| CAD | build123d + OpenCascade | 唯一权威 B-Rep 内核 |
| CAD IR | 自有 DesignSpec + FeatureGraph | 安全、可审计、可重建 |
| 查看器 | three-cad-viewer + three-mesh-bvh | 拓扑选择、测量、截面和增量更新 |
| 网格检查 | trimesh；Manifold 按需 | Print Doctor 与规范化 |
| 3MF | lib3mf | 正式写入、元数据与回读验证 |
| 切片 | PrusaSlicer CLI Adapter | 可选打印时间与耗材估算 |
| LLM 结构化输出 | Instructor + Pydantic Schema | 需求、ChangeSet 与解释 |
| 评测 | CADGenBench 思路 + 自有真值集 | STEP、尺寸、接口与 DFM 门禁 |

首版只维护 build123d 这一套权威内核。CadQuery 可以保留为后续兼容后端，但不进入 MVP 双栈。

## 第一个验收纵向切片

输入：

> 设计一个模块化未来手枪 CAD 样机。整体长度 230 mm，主体高度 54 mm，枪管模块长度 120 mm，握把角度 15°，最小壁厚 2.5 mm；主体、枪管、握把、导轨、瞄具和供弹模块必须有稳定接口与独立参数。

系统必须产出 DesignSpec、FeatureGraph、有效 B-Rep、STEP/3MF/STL/GLB、关键尺寸验证、DFM 报告、工程假设和版本记录。涉及爆压、热载荷、材料强度或法规的条件必须明确标为尚未验证，不能伪装成认证结论。随后接受修改：

> 将上导轨延长 12 mm，握把角度改为 17°，保持枪管、主体和供弹模块接口不变。

修改必须先生成 ChangeSet，保护锁定接口，创建子版本，重建并重跑 DFM，不能覆盖父版本。L 型支架保留为 CAD 内核的简单几何校准样本，不再作为产品主场景。

## 风险与责任边界

设计风险分为：

- `R0`：装饰件、无承载；
- `R1`：普通功能件；
- `R2`：明显承载、热、电气或运动结构，需要警告与人工确认；
- `R3`：安全关键、受监管或受限用途，阻止生成或生产导出。

系统不得声称“结构安全”“已经认证”“符合标准”或“可直接承载”，除非未来具有完整载荷、材料、边界条件、仿真、实测和认证证据。

## 开始工作

当前仓库仍运行迁移前基线：

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"
npm run m6:gate
```

本地 Agent、Vite 开发壳与 Tauri 桌面窗口的区别及完整命令见 [docs/OPERATIONS.md](docs/OPERATIONS.md)。浏览器 `127.0.0.1:5173` 只是 Vite 开发壳，交付产品路径是 Tauri 本地桌面窗口。

## 文档地图

- [路线图与实施计划](docs/IMPLEMENTATION_PLAN.md)：阶段、PR 顺序、退出门与近期行动。
- [系统设计](docs/DESIGN.md)：领域模型、架构、API、数据、CAD Runtime、DFM 与安全设计。
- [操作手册](docs/OPERATIONS.md)：安装、运行、验证、数据管理、故障处理与发布检查。
- [架构决策](docs/ADR/)：产品转向、CAD 内核、FeatureGraph 安全、许可证和旧数据迁移。
- [执行证据](docs/evidence/)：R0 基线与后续阶段门禁记录。
- [API（旧基线）](docs/API.md)：当前 `/api/weapons` API，仅用于迁移期间。
- [数据库（旧基线）](docs/DATABASE.md)：当前 weapon 领域数据库，仅用于迁移参考。
- [Schema（旧基线）](docs/SCHEMAS.md)：当前 Weapon 合同，待新合同落地后冻结归档。
- [第三方许可证](docs/THIRD_PARTY_LICENSES.md)：现有依赖；CAD 新依赖必须在引入 PR 中补齐。

## 贡献约束

- 不在 `asset_store.py` 继续新增 CAD、DFM、切片或导出职责。
- 不把 Weapon 类型机械重命名成 Design 类型。
- 新合同变更后必须重新生成 Python、TypeScript 与 OpenAPI 产物。
- 所有耗时操作统一进入 Job，并保留幂等、取消、恢复和事件 replay。
- 所有生产导出必须能追溯到合同哈希、编译器/内核版本、Profile 版本和验证结果。
- 第三方依赖必须固定版本、记录许可证、生成 SBOM，并说明是直接依赖、独立进程还是仅参考。

完整的执行顺序以 [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) 为准。
