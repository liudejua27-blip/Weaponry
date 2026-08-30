# Weaponry 穿越火线武器 Agent-native DCC 产品宪章

- Status: current product authority
- Effective date: 2026-08-29
- Delivery horizon: one month for V1 acceptance cohort
- Product name: Weaponry
- Runtime lineage: ForgeCAD Rust Runtime

> **Knife-first override:** 十天首交付范围由 ADR-0030 与
> `WEAPONRY_KNIFE_10_DAY_DELIVERY_PLAN.md` 收缩为穿越火线刀类。允许固定版本、固定插件、
> closed-job 的隔离 Blender 内部原型；它不改变 Rust Runtime 单写者和最终真值边界。

## 1. 唯一产品目标

Weaponry 是由 Codex 操控、Rust Runtime 唯一写入的武器专用 Agent-native
DCC。当前 P0 只主攻为穿越火线合作项目生成和迭代高质量、非功能性游戏武器视觉资产。
汽车、建筑、机器人、通用 CAD、独立聊天 Agent 和多模型 Provider 均不是本月产品目标；
它们只能作为测试夹具、历史能力或未来扩展存在，不能抢占主线资源。

“高质量”不是 Tool 数量、Schema 数量、GLB 可打开或渲染好看。它要求同一候选血缘闭合：

`授权参考 → AuthoringMesh → Modifier/Evaluation → High → editable Low → Hero UV → Cage/Bake → PBR → FPS/inspect/ADS → engine validation → independent artist review → explicit approval`

任何前置门失败，后续材质、灯光、VFX 或截图都不能把候选提升为高质量资产。

## 2. 一个月交付定义

V1 必须以至少一个已授权穿越火线武器和一个原创 control weapon 形成同 cohort
验收。二者必须使用同一组通用网格操作、Modifier、UV/Bake、材质、相机和导出路径；
不得为验收武器写入只适用于该资产的内核分支。

交付状态必须拆开记录：

- `AUTHORING_RUNTIME_PASS`
- `GAME_READY_SURFACE_PASS`
- `VISUAL_REFERENCE_PASS`
- `HUMAN_ARTIST_ACCEPTED`
- `ENGINE_VALIDATED`
- `PACKAGED`
- `RELEASED`

只有全部必需标签在同一候选 hash 上通过，才能称为穿越火线武器 V1 已交付。

## 3. 开放 Action Space

Weaponry 不通过无限增加顶层 Tool 扩大建模能力。核心表达空间来自可组合命令代数：

- stable-ID vertex/edge/face/loop/Part/MaterialZone selection；
- adjacency、boundary、normal、angle、visibility 和 semantic query；
- split、move、extrude、inset、bevel、bridge、merge、dissolve、loop cut/slide；
- transform、mirror、array、boolean、solidify、subdivision、weighted normal；
- 多命令原子事务、确定性 journal、失败全回滚；
- 原始 AuthoringMesh 与可丢弃 EvaluatedMesh 分离；
- typed Geometry Nodes 子图和经审计的扩展 Provider。

Receiver、stock、rail、optic、magazine 等武器概念属于宏、语义预设和质量 Benchmark，
不得成为只能服务某一把枪的内核操作。

任意 Python、JavaScript、shell、Blender Add-on、URL 或未审计 native plugin 不进入
Runtime 真值。开放表达空间与任意代码执行不是同一件事。

## 4. 产品架构

```text
Codex Desktop / CLI
        |
        v
thin typed MCP surface
        |
        v
Rust Runtime (sole writer)
  |-- DesignSession / authorization / checkpoint / approval
  |-- AuthoringMesh / selection / atomic command journal
  |-- ModifierGraph / DependencyGraph / EvaluatedMesh
  |-- candidate / immutable version / Store / CAS
        |
        +--> bounded Geometry / High / UV / Bake / Render Workers
        |
        +--> read-only Viewer / AOV / compare / critic / engine receipt
```

MCP 不拥有数据库；Viewer 不写产品状态；Worker 不监听网络、不执行任意脚本；
所有永久修改都经过 `preview → prepare → validate/readback → evidence → user approval → confirm`。

## 5. 模块优先级

P0 必须完成：

1. AuthoringMesh selection/query 和多操作原子事务公共 Contract/MCP/Store/CAS；
2. ordered Modifier/Evaluation Graph 和局部 dirty recompute；
3. hard-surface 通用编辑与 robust Boolean/Subdivision/Normals；
4. High/Low、artist-editable topology、Hero UV、Cage 和零隐藏 fallback Bake；
5. Material Layer Graph、PBR channel readback 和 provenance；
6. first-person、inspect、ADS 相机与遮挡/握持可读性检查；
7. LOD、collision、socket、animation/export 和商业引擎验证；
8. 固定视图/AOV/参考比较、独立武器美术人审和批准回执。

P0 不做：完整 Blender UI、通用 Sculpt/NURBS/Geometry Nodes parity、角色 DCC、通用场景
编辑器、模型 Provider、产品内聊天和任意公开脚本插件。刀类所需的 Sculpt-like pass、
curve/NURBS subset、typed node graph、retopo、UV/Bake 与动画必须达到刀类任务覆盖；实现可由
Rust 产品轨与 ADR-0030 定义的隔离 Blender 内部原型共同完成。

## 6. Tool 策略

公共 Tool 以任务语义分为五组：

1. `observe/query`：读取授权、参考、拓扑、选择、Modifier、候选和证据；
2. `author`：预览/准备原子编辑事务；
3. `evaluate`：求值 Modifier、High/Low/UV/Bake/PBR 和固定渲染；
4. `review`：AOV、compare、critic、human decision 与 engine receipt；
5. `version/deliver`：checkpoint、confirm、restore、export。

V1/V2/V3 和 subject-specific 工具先进入 legacy profile，再完成 durable replay migration，
最后删除。Tool 数量下降不是目标；同一通用命令无需新增 Rust 内核即可表达更多武器形体才是目标。

## 7. Skill 策略

所有 active Skill 必须服务上述穿越火线武器链路，并按版本迁移：

- intake/authorization/reference；
- silhouette/primary form；
- hard-surface authoring/modifier；
- high/low/UV/cage/bake；
- PBR/material/decals/wear；
- FPS presentation/animation/engine delivery；
- evidence/review/recovery。

修改 hash-bound Bundle knowledge 必须升版本、更新 manifest/lock/hash/SBOM/Benchmark，
不得直接改写已发布 Bundle 或 archive 内容。仓库级 Codex Skill 只负责编排，不成为 Runtime 真值。

## 8. 穿越火线授权与安全边界

商业合作由用户声明，但每个私有参考、源模型、贴图、Logo、命名和交付目标仍必须绑定：

- asset/source content hash；
- 权利方或允许用途声明；
- 可修改和可导出范围；
- 目标项目与接收方；
- 到期、撤销和 provenance 状态。

缺少授权记录时只能使用原创 control asset。Weaponry 只生成非功能性游戏/影视视觉资产，
不输出现实可制造武器尺寸、制造图、加工步骤、材料配方、性能或操作建议。

## 9. 文档权威和历史保护

阅读优先级：

1. 本宪章；
2. ADR-0029；
3. `WEAPONRY_ONE_MONTH_DELIVERY_PLAN.md`；
4. `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`；
5. 当前状态、任务索引和模块/合同文档；
6. 旧 ADR、研究报告、历史计划和 evidence。

旧文档与本宪章冲突时，以本宪章为当前产品方向；历史证据仍按其产生时的原始内容解释，
不能追溯改写成穿越火线质量 PASS。

## 10. 当前真实状态

截至 2026-08-29，Weaponry 仍不是可交付的高质量穿越火线武器 DCC。现有结构、GLB、
UV/PBR 和渲染切片不能替代公共 AuthoringTransaction、完整 Modifier Graph、正式
High→Low Cage Bake、商业引擎验证和独立武器美术人审。当前质量继续为
`QUALITY_TARGET_NOT_MET / commercial=NOT_PROVEN`，直到同候选验收链闭合。
