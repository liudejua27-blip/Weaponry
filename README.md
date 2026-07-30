# ForgeCAD / Forge Studio

ForgeCAD 是 Forge Studio 的代码仓库：一个面向零基础用户的、本地优先、类别开放的 AI 视觉 3D 设计工作台。用户上传什么对象的授权图片并描述什么对象，目标系统就保持该对象身份，按部件选择程序化、形变或本地混合表示；Rust 统一校验、编译、回读和版本化为可编辑 GLB/PBR。

当前是本机 Alpha，不是生产软件，也不是“任意图片一键生成任意高质量 3D”的已完成产品。现有可靠生成路径仍主要是机械臂/机械硬表面；角色、生物、植物、环境和混合表示尚未实现。ADR-0022 已取消产品类别 allowlist，但目标变化不等于当前能力完成。所有结果带非制造说明；未来武器只用于虚构游戏美术资产、影视道具和非功能展示模型，不输出可用于现实制造武器的精确图纸、功能机构、制造尺寸、材料配方或加工步骤；其他对象也不提供结构、安全、适航、医疗、控制或认证结论。

## 当前一句话状态

ForgeCAD 已经具备可靠的资产生命周期和一条真实 DeepSeek→Rust lowering→GLB→唯一预览→确认→`ActiveDesignSnapshot`→导出闭环。E005-R1 已新增正式紧凑作者源，把 typed parameter、宏/repeat、高层机械几何、刚性 Part 层级、Surface 与 detail motif 接入同一受限编译链；当前瓶颈转为参考图与候选渲染之间的真实视觉比较/一次 typed visual patch、production-review PBR 和独立真人门，而不是继续增加整机模板。

## 当前架构

```text
自然语言 / 授权参考 / 当前资产
        ↓
千问视觉理解与证据合同
        ↓
Rust-sealed SubjectProfile / VisualFeatureContract / RepresentationPlan
        ↓
DeepSeek ForgeVisualAuthoringIntent@1 / ForgeVisualProgram
        ↓
Rust app-server / forgecad-core
  合同、预算、lowering、版本、CAS、Snapshot、质量、恢复
        ↓
ForgeVisualProgram@1
  ShapeProgram + AssemblyGraph + Material Zone + Surface Program
        ↓
RestrictedGeometryExecutor（迁移期 Python sidecar）
  受限几何、PBR、GLB、readback
        ↓
单一 Three.js renderer
  八视图检查、唯一未保存结果、preview → confirm → export
```

Rust 是产品状态、Provider 生命周期、SQLite/CAS 和版本真值的唯一所有者。Python 只执行 capability-gated 的受限几何请求，不接收 Provider Key、数据库路径或 Snapshot 写权限。前端只维护一个 WebGL renderer/context。

## 已实现的主要能力

- Rust-owned Thread/Turn/Item/Approval、DeepSeek Provider Gateway 和 Product Tool Action Loop；
- `ForgeVisualAuthoringIntent@1` → `ForgeVisualProgram@1` → `ShapeProgram@1` / `AssemblyGraph@1` 的受限 lowering；
- 受限 `box`/`cylinder`、wedge/capsule、Profile/Extrude、Revolve、Loft、Sweep、阵列、有限布尔和表面程序；
- GLB、五通道 PBR、Material Zone、真实 readback、内容寻址对象和双档 preview/production 工件；
- 唯一结果、ChangeSet preview→confirm、不可变 AssetVersion、undo/redo、重启恢复和 GLB 导出；
- `ActiveDesignSnapshot` 对活动版本、选择、质量、预览和导出的统一绑定；
- 单图、多视图 contact sheet 和严格 GLB 的只读参考证据链；
- 同一资产连续语言修改、几何/材质锁、八视图和六成员 ForgeAssetPackage 后端闭环；
- 单一 Three.js canvas 的 docked/focus 工作台；
- 旧 Weapon/Concept 数据的只读兼容和显式转换。

以上证明机械硬表面工程闭环，不证明通用类别自由生成、照片级外观或独立真人 `4/5` 视觉验收。

## 当前未完成

- 机械臂黄金路径尚未达到 M108B 的独立真人视觉门；
- 当前产品 UI 的 `ForgeVisualAuthoringIntent` 仍主要覆盖机械臂 serial/parallel 受审家族；E005-R1 author source 尚未宣告为用户可用通用生成入口；
- E005-R2 的 sealed reference + candidate render visual patch 和 R3 production-review PBR/真实图片输入尚未完成；
- 30 条冻结未见多模态硬表面任务仍为 0 formal run，逐任务真人质量基准未完成；
- 汽车、飞机和未来道具尚未进入与机械臂同等级的程序化视觉主链；
- 自由网格编辑、B-Rep、STEP、工程碰撞/运动学、DFM 和认证不在当前能力范围；
- 广泛多客户端并发 E2E、跨平台非空 sidecar、签名、公证、全新机安装和升级仍是发布阻断。

为什么长期“做不完”、与 `img2threejs` 的差异、升级点和收敛路线见 [项目收敛与外部基准审计](docs/AGENT_CURRENT_ISSUES_AUDIT.md)。

2026-07-29 已接受[通用参考条件 3D Agent 与能力沙箱](docs/ADR/0022-universal-reference-conditioned-3d-agent.md)：类别从入口开放，机械硬表面降为当前成熟表示与回归分布；`SubjectProfile → RepresentationPlan → UniversalAssetSource` 统一程序化、形变和本地混合表示。随后 [ADR-0023](docs/ADR/0023-deepseek-qwen-only-ai-provider-policy.md) 将运行时 AI Provider 永久限定为 DeepSeek 与千问：不使用 Fal 或第三方远程 Mesh API，真正的 GLB/PBR 由本地受限编译链产生。

## 下一条唯一主线

```text
U001 通用产品/文档/任务决策
→ U002 SubjectProfile + VisualFeatureContract + RepresentationPlan
→ U003 UniversalAssetSource + 通用 detail/material/projection（已完成当前程序化切片）
→ U004 DeepSeek/千问驱动的 procedural + deformable + local hybrid
→ U005 跨类别真实未见集 + 1+1 时间/成本 + 真人盲评
→ 设计伙伴验证与发布
```

当前不再通过新增固定整机模板、增加三角形数量或提高纹理分辨率来替代设计语言建设。

当前进度：U002 已完成类别开放理解、视觉合同、逐部件表示规划和 typed limitation；U003 已完成 Rust 派生的统一资产源、外观证据合同与当前程序化结果的 GLB/readback/固定视图 exact-lineage。只有验证后的机械臂程序化 capability 可执行；下一唯一任务是 U004，角色、生物、植物、建筑等尚不能据此宣称已生成。

## 本机开发

安装依赖：

```bash
npm install
python3 -m venv .venv
.venv/bin/pip install -e "apps/agent[dev]"
```

验证并启动开发版：

```bash
script/build_and_run.sh --verify
```

当前开发 supervisor 可使用 Python sidecar；发布包的跨平台 sidecar 和签名状态必须以 [打包说明](docs/PACKAGING.md) 与 `npm run release:packaging-readiness` 为准。该命令当前应继续拒绝 Intel macOS、Windows 和 Linux 的空 sidecar；这是已知发布阻断，不应通过删除目标或放宽检查消除。

常用验证：

```bash
npm run agent:check
npm run contracts:types:check
npm run desktop:typecheck
npm run desktop:build
npm run desktop:r3-concept-workbench-smoke
npm run release:docs-walkthrough
npm run repository:integrity
```

单项视觉主链 Gate 以 [任务索引](docs/CODEX_TASK_INDEX.md) 中当前任务卡为准。CI 绿色只证明对应提交和对应 Gate，不自动证明当前脏工作区、真实 Provider 质量或生产发布已通过。

## 文档入口

按以下顺序进入，避免把历史目标当成当前能力：

1. [文档地图](docs/DOCUMENTATION_MAP.md)
2. [文档状态账本](docs/DOCUMENTATION_STATUS.md)
3. [当前交接](docs/CODEX_HANDOFF.md)
4. [执行计划](docs/CODEX_EXECUTION_PLAN.md)
5. [原子任务索引](docs/CODEX_TASK_INDEX.md)
6. [权威状态合同](docs/AUTHORITATIVE_STATE.md)
7. [本机 Alpha 用户指南](docs/USER_GUIDE.md)
8. [目标设计](docs/DESIGN.md)
9. [Luna Goal 模式持续执行指南](docs/LUNA_GOAL_EXECUTION_GUIDE.md)

核心决策：

- [产品定义](docs/PRODUCT_DEFINITION.md)
- [程序化视觉 MVP：ADR-0019](docs/ADR/0019-programmatic-visual-program-mvp.md)
- [轻量外观优先 3D Agent：ADR-0020](docs/ADR/0020-lightweight-appearance-first-3d-agent.md)
- [Codex 式设计工作区：ADR-0017](docs/ADR/0017-codex-design-workspace-visual-convergence.md)
- [项目收敛与 img2threejs 对比](docs/AGENT_CURRENT_ISSUES_AUDIT.md)
- [GitHub 参考与采用边界](docs/AGENT_GITHUB_REFERENCE_ARCHITECTURE.md)
- [能力—Gate 矩阵](docs/evidence/CAPABILITY_GATE_MATRIX.md)
- [兼容迁移与旧运行时边界](docs/COMPATIBILITY_MIGRATION.md)

## 仓库约束

- Core 只拥有通用 Project、Assembly、Part、Shape、Material、Joint、Version 和 Tool；领域语义进入版本化 Domain Pack；
- 不执行模型生成的任意 Python、JavaScript、shell、URL 或文件路径；
- 永久修改必须先 preview，再 confirm，再创建不可变子版本；
- GLB、readback、版本和导出必须保持 exact lineage；
- Provider Key 只进入受保护的 secret/稳定签名后的 Keychain；
- AI Provider 只允许 DeepSeek 与千问，第三方聚合图像/网格 API 无运行时入口；
- legacy 代码只有在启动链、迁移、发布门和旧库只读转换均完成后才能删除；
- 用户文档只写已验证能力，目标能力只写入设计、计划和任务索引。

修改仓库前必须先阅读 [AGENTS.md](AGENTS.md)。
