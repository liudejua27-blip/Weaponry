# ForgeCAD Codex / Luna 工作规则

本文件适用于整个仓库。2026-08-07 起，所有旧 Agent、Provider、U004 和工作台指令由 ADR-0025 取代。

## 1. 产品定义

ForgeCAD 是由 Codex 调用的本地、可验证、可回退 3D Runtime，不是内置大模型的独立 Agent 应用。

- 用户在 Codex Desktop、Codex CLI 或 Codex IDE 中对话和上传授权参考；
- Codex 是外部大脑，负责理解、规划、视觉推理和工具编排；
- ForgeCAD 是身体，负责 typed 几何、UV、PBR、纹理、材质、渲染、质量、Skill、版本、局部修改、爆炸图和导出；
- ForgeCAD Desktop 只查看项目、候选、部件、参考、固定视图、质量、Job 和版本，不提供聊天、图片上传、模型选择、Provider 配置或 API Key；
- P0 只支持和验收 Codex，不内置 OpenAI、DeepSeek、千问或其他模型调用；
- MCP `stdio` 是 Codex 入口，Rust Runtime 是唯一产品状态写者。

“Codex-only”指支持范围，不是模型身份认证。不得用 `client_name == codex` 作为安全边界。图片附件必须通过真实 Codex 客户端证明字节进入 ForgeCAD CAS；没有证据时标为 unavailable。

当前是破坏性重构后的迁移期，不是可用的新产品。MCP002 contracts/Store/Runtime/IPC 基础层与 MCP003 本地只读 MCP adapter 已通过 Gate；官方 conformance、真实三宿主连接和 packaged E2E 未完成前，不得宣称“Codex 已可生成高质量 3D”。

## 2. 唯一权威阅读顺序

开始任何任务前完整阅读：

1. `docs/DOCUMENTATION_MAP.md`
2. `docs/DOCUMENTATION_STATUS.md`
3. `docs/CODEX_HANDOFF.md`
4. `docs/ADR/0025-codex-only-mcp-3d-runtime.md`
5. `docs/RESET_MIGRATION_PLAN.md`
6. `docs/CODEX_EXECUTION_PLAN.md`
7. `docs/CODEX_TASK_INDEX.md`
8. `docs/AUTHORITATIVE_STATE.md`
9. `docs/LUNA_GOAL_EXECUTION_GUIDE.md`（Luna/Goal 执行时必读）
10. 与任务直接相关的合同：MCP、Codex、Compiler、Viewer、Skill、Schema、测试或打包文档。

旧 ADR、U004 总图、Provider、Domain、Mechanical、Module 和 Compatibility 文档已从当前树删除，没有执行权威。不得从 Git 历史恢复旧产品路径来让测试通过。

## 3. 强制实施顺序

实施顺序固定为 `FGC-MCP000 → MCP001 → ... → MCP013`，详见任务索引。同一时刻只领取一个原子任务。

`FGC-MCP001` 和 `FGC-MCP002` 已在 reset 分支完成；当前第一项代码任务是 `FGC-MCP003`：

1. 完整阅读 MCP001/MCP002 evidence 和当前工作树；
2. 实现 MCP stdio 的 resources/list、resources/read、只读工具、annotations、合同/版本协商和 fail-closed 错误；
3. 为 Codex Desktop、CLI、IDE 生成不含 secret/绝对路径的配置基线；
4. 运行官方协议快照，并在可用的 Codex Desktop/CLI/IDE 主机中执行连接/断开/版本不兼容 Gate；当前仓库只记录本地 adapter PASS 与真实主机 NOT_RUN；
5. 更新状态、能力矩阵、handoff 和 MCP003 evidence。

不得在当前脏 `main` 上直接删除。不得跳过 MCP001 继续扩展旧工作台或修 Provider。

## 4. 不可违反的架构约束

- `forgecad-runtime` 是 SQLite/CAS/项目/候选/版本/Job/Skill 的唯一写者；
- `forgecad-mcp` 是无数据库的薄 `stdio` 适配器；
- Viewer 只有 read model 和临时 camera/selection/isolation/explosion 状态；
- Worker 只接受受限 typed 内部协议，不监听网络，不执行任意脚本；
- 同一候选的 Geometry/Appearance/Render/Quality/Export 共享 ID、hash 和 lineage；
- `ActiveDesignSnapshot` 是当前项目的单一状态投影；
- 所有永久修改先 prepare，后编译/回读/质量，再由用户在 Codex 批准，最后 confirm；
- 确认创建不可变子版本；restore 创建新版本，不改写历史；
- 大文件进入 CAS，事件/日志只保存引用；
- 新 Runtime 使用全新 V1 数据库，不自动打开旧 Library；旧数据只读保存并由一次性工具显式导出；
- 无任意 Python、JavaScript、shell、URL、文件路径、环境变量或 secret 进入几何真值；
- 不使用 Provider Registry，不读取或存储 Codex/OpenAI/DeepSeek/千问 API Key。

## 5. Skill 约束

Skill 必须同时包含：

`知识 + typed Schema + Recipe DAG + 受限 Operator + Validator + 材质/资产 + Benchmark + LICENSE/NOTICE/SBOM + provenance + signature`

P0 Bundle 只含声明式内容；可执行 Operator 必须是产品预注册实现。签名不等于安全或质量，仍须通过合同、权限、预算、许可证、恶意输入和 Benchmark Gate。GitHub 仓库不能直接安装为 Skill。

## 6. 高质量定义

“高质量”至少需要同一 candidate hash 的：

- 合同、预算、几何和严格 GLB readback；
- 语义 Part/MaterialZone/source map；
- 多视图轮廓与比例；
- UV/tangent、PBR 通道、纹理、材质和 provenance；
- 固定相机的 beauty/depth/normal/AO/part-ID/material-ID/wireframe/UV-stretch/silhouette；
- 参考比较、Codex typed visual review 和独立真人门；
- preview/export/restart 同一版本和 hash。

Skill 安装、材质包、单张截图、GLB 可打开、本地 smoke 或 Codex 自评不能替代这些证据。

## 7. Blender 与外部项目边界

可以学习 Blender 的 data-block、Modifier/Geometry Nodes、Principled PBR、UV/UDIM/Bake、AOV、OCIO、Asset Browser 和 Outliner；不能把 `.blend`、任意 Blender Python 或 Blender 内部状态变成产品真值。

外部项目按 Library、isolated Worker、Asset 或 Reference-only 分离采用。必须固定 commit、审许可证和例外、生成 SBOM、运行恶意输入/资源/确定性/跨平台 Benchmark，并保留退出方案。未经批准不得下载权重、执行安装脚本或整仓复制。

## 8. 安全范围

ForgeCAD 面向合法的非功能性 3D 视觉资产。未来虚构武器只限游戏美术、影视道具和展示；项目不生成现实可制造武器、制造图、制造尺寸、材料配方、加工流程、性能或操作建议。汽车、飞机、建筑、角色和机械结果不提供结构、安全、适航、医疗、动力学或认证结论。

参考图片和资产必须由用户有权使用；导出保留 license/provenance。日志、receipt、MCP 输出和包内不得泄露 secret、prompt、图片原始字节、本机用户名或绝对路径。

## 9. 任务规则

任务开始：

- 记录任务 ID，确认依赖和唯一 `in_progress`；
- 运行 `git status -sb`、`git diff --check`；
- 阅读任务代码入口、Schema 和 Gate；
- 记录基线结果，保护用户未提交修改；
- 若任务含删除，先满足重置恢复门。

任务结束：

- 只在退出条件全部满足时标为 `done`；
- 成功、失败、阻断、未运行分别记录；
- 更新任务索引、状态账本、能力矩阵和 handoff；
- 不用“基本完成”“应该可用”替代证据；
- 除非用户明确要求，不 commit、merge 或 push。

## 10. 基线 Gate

文档/合同变更至少运行：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
git diff --check
```

代码重置后，CI 只保留 contracts、core/store、geometry/render workers、MCP conformance、Codex E2E、Viewer、quality、packaging、安全和许可证 Gate。旧 Provider/U004/workbench Gate 必须删除，不得放宽新 Gate 来换绿色。

## 11. 完成定义

只有 `FGC-MCP001`–`FGC-MCP013` 全部完成，且真实 packaged Codex → MCP → 参考字节 → candidate → Viewer → 局部修改 → 用户批准 → 不可变版本 → 回退 → 爆炸图 → 导出闭环通过，产品迁移才完成。此前用户指南只描述当前可用的诊断或 Viewer 能力。
