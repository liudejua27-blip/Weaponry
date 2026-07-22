# ForgeCAD Codex 工作规则

本文件适用于整个仓库。后续 Codex 在计划、修改、测试或汇报前必须先阅读本文件。

## 1. 产品定义

ForgeCAD 是面向零基础用户的轻量通用机械概念 3D Agent。首批领域包是未来武器概念道具、汽车、飞机和机械臂。

当前是本机 Alpha，不是生产软件。不得把目标设计、legacy Weapon/Unity 证据或确定性 smoke 描述成已经完成的通用产品能力。

未来武器结果仅限虚构游戏美术资产、影视道具和非功能展示模型。不得增加现实武器制造图、功能机构、制造尺寸、材料配方、加工步骤或性能建议。汽车、飞机和机械臂同样不提供安全、适航、结构、动力学或认证结论。

## 2. 必读顺序

开始任何实现任务前按顺序阅读：

1. `docs/DOCUMENTATION_MAP.md`：确认唯一权威和文档生命周期；
2. `docs/DOCUMENTATION_STATUS.md`：当前文档状态账本、能力标签和已知阻断；
3. `docs/CODEX_HANDOFF.md`：当前工作区和已知失败；
4. `docs/CODEX_EXECUTION_PLAN.md`：阶段顺序和退出条件；
5. `docs/CODEX_TASK_INDEX.md`：选择一个原子任务；
6. `docs/AUTHORITATIVE_STATE.md`：版本、选择、质量和导出真值；
7. `docs/USER_GUIDE.md`：当前真实用户能力；
8. `docs/DESIGN.md`：目标架构；
9. 与任务直接相关的 API、Schema、测试或操作文档。

`docs/legacy/` 只用于兼容和迁移。不得从 legacy 文档推导新产品功能。

需要外部参考、插件或 Skill 时，先读 `docs/AGENT_GITHUB_REFERENCE_ARCHITECTURE.md` 和 `docs/AGENT_PLUGINS_SKILLS_DESIGN.md`。不得整套复制通用 Agent/CAD 项目，也不得让零基础用户安装开发 Skill、本地神经 3D 或 DCC 插件。

## 3. 强制执行顺序

除非用户明确改变优先级，必须按下列顺序推进：

1. `ActiveDesignSnapshot` 单一状态真值；
2. 未知/含糊领域澄清；
3. 统一版本、选择、质量、撤销/回退和导出；
4. 修复工作台 E2E，并把 G1–G7 纳入 CI；
5. 拆分 `CadWorkbenchPanel` 和建立前端状态机；
6. 扩展轻量 ShapeProgram/Geometry Worker；
7. 实现 Agent 多视图概念渲染；
8. 扩充视觉材质和领域组件；
9. 完成 packaged sidecar、安装、恢复、签名和发布。

不得跳过 1–4 直接增加大型 UI、更多导出格式或复杂几何。

`FGC-S001`–`FGC-S008`、`FGC-D001`–`FGC-D003`、`FGC-T001`–`FGC-T003`、`FGC-B001`–`FGC-B002`、`FGC-P001`、`FGC-P007`、`FGC-F001`–`FGC-F006`、`FGC-F026`、`FGC-G801`–`FGC-G808`、`FGC-R001`–`FGC-R002`、`FGC-R007A`–`FGC-R007B`、`FGC-M101`–`FGC-M107`、`FGC-M108A`、`FGC-C101`–`FGC-C108`、`FGC-K001`–`FGC-K003`、`FGC-A005`、`FGC-V003` 已完成。A005 提供 Rust-owned Skill 生命周期、受限 `SurfaceAdornmentProgram@1`、ChangeSet preview→confirm、128/1024 两档五通道 PBR 和单视口 UI；它不增加 ShapeProgram operation 或任意执行能力。R007A 将授权图片/GLB封装为 Rust-owned 只读证据；R007B 已用单图、多视图 contact sheet、严格 GLB readback 三类 exact-lineage packaged 工作台证据完成参考→Design Surface/Recipe/Material Zone/A005→新 GLB 的工程闭环，但明确不证明视觉相似度。V003 由 Rust 执行一次完整 synthesis、13 项 code-owned v2 Gate 和最多两次同意图原位修复，只产生一个未保存 `SingleResultDecision@1`；用户确认才创建原子资产版本。K003 已让 Rust app-server/core 单一拥有 Agent 与产品状态；Python 只保留 capability-gated `RestrictedGeometryExecutor`。C108 已将 service-display 深化为 19,776-triangle preview 与 101,248-triangle/120-primitive/1K PBR production，并完成 packaged 唯一结果→A005 V2→Snapshot/导出→重启恢复；实际截图仍未达到目标图。`FGC-M108B` 仍为 `blocked`：四领域正式 production Recipe kit 和三位独立真人逐领域 `4/5` 未完成；M109 的 2K/压缩纹理与设备分级继续等待它。F026 已完成 Codex 式 shell 和单 renderer `docked | focus`，并移除三方向 UI。不得把 C104/G808 擅自扩展为工程装配约束、自由参数或新几何能力。

## 4. 任务粒度

一次只领取 `docs/CODEX_TASK_INDEX.md` 中一个可独立验收的任务。任务开始时：

- 记录任务 ID；
- 检查依赖是否完成；
- 阅读列出的代码入口和合同；
- 运行任务前基线命令；
- 保留用户已有未提交修改。

任务结束时：

- 实现代码、迁移、类型和测试；
- 更新任务状态和受影响文档；
- 运行任务 Gate；
- 记录通过、失败和未运行项；
- 不用“基本完成”代替退出条件。

## 5. 当前真值和目标真值

当前代码仍同时存在：

- legacy `ConceptVersion/ModuleGraph`；
- 新 `AgentAssetVersion/AssemblyGraph`。

当前状态不一致是 P0 缺陷。目标是 `ActiveDesignSnapshot@1`。在目标实现前：

- 不把两套 `vN` 合并显示；
- 不按导出格式隐式切换版本链；
- 不把 localStorage 当作生产版本头；
- 不把旧质量报告附着到新 Agent 资产；
- 版本不一致时阻止导出。

## 6. 架构约束

- Core 使用通用 Project、Assembly、Part、Shape、Material、Joint、Version 和 Tool；
- 领域语义进入版本化 Domain Pack；
- ShapeProgram 不执行任意 Python、JavaScript、shell、URL 或文件路径；
- 所有永久修改先 preview，再 confirm，再创建不可变子版本；
- 一个工作台只能有一个 WebGL renderer/context；
- Provider Key 只进入 Keychain 或权限受限的 secret file；
- 大文件进入内容寻址对象库，不进入事件和日志；
- 新 API 使用 `/api/v1/agent`，legacy API 只读或显式转换。

## 7. 文档状态规则

每项能力只能标为：

- `已实现`：代码和当前 Gate 通过；
- `部分实现`：列出已完成与未完成子能力；
- `目标设计`：没有当前实现证据；
- `legacy`：只服务兼容；
- `blocked`：退出条件明确失败。

用户指南只能包含已实现能力。目标能力写入 DESIGN、EXECUTION_PLAN 或 TASK_INDEX。修改用户能力时同步更新 `docs/evidence/CAPABILITY_GATE_MATRIX.md`。

## 8. 基线验证

文档或合同变更至少运行：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run agent:check
git diff --check
```

Agent/后端变更还需运行相关 G1–G7 smoke 和 `contracts:types:check`。前端变更至少运行 typecheck、build、工作台 E2E；Tauri 变更还需 cargo check 和原生验证。

当前发布阻断不能被删除或放宽：

- `desktop:r3-concept-workbench-smoke` 的 Snapshot、preview、quality、undo/redo、重启与导出版本链断言必须保留并继续扩展；
- `release:packaging-readiness` 空 packaged sidecar。

## 9. 工作区和 Git

- 先运行 `git status -sb` 和 `git diff --check`；
- 当前仓库可能有大量用户未提交修改，禁止 reset、checkout 或覆盖无关文件；
- 不删除旧数据、迁移或兼容 fixture 来让测试通过；
- 除非用户明确要求，不提交、不合并、不 push；
- CI 绿色只证明其对应 commit，不证明当前脏工作区。

## 10. 完成定义

任务只有在以下条件同时满足时才完成：

- 任务退出条件全部满足；
- 相关自动测试通过；
- 失败路径和重启/幂等边界有测试；
- 文档与当前实现一致；
- 没有泄露密钥、绝对路径或外部付费调用；
- 没有把 legacy 证据当作新 Agent 证据；
- handoff 记录了 commit/工作区、命令、结果和剩余阻断。

详细完成定义见 `docs/CODEX_DEFINITION_OF_DONE.md`。
