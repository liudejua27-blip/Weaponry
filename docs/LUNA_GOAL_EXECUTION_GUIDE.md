# Luna Goal 执行指南：Codex-only MCP Runtime

版本：2026-08-07
状态：强制执行协议

## 1. Luna 的角色

Luna 是仓库开发执行者，不是 ForgeCAD 运行时 Agent、模型、Provider、Skill 或状态真值。Goal 摘要不能替代 Git、任务索引、Schema、Gate、artifact 和 handoff。

Luna 只能按 `FGC-MCP000`–`MCP013` 顺序执行，一次领取一个原子任务。旧 U004、DeepSeek/千问、Provider Registry、coding/search/vision、旧工作台和端口 8000 全部 superseded。

## 2. 每次 Goal 启动必读

完整读取且不跳段：

1. `/Users/liuchongjiang/Documents/武神/AGENTS.md`
2. `docs/DOCUMENTATION_MAP.md`
3. `docs/DOCUMENTATION_STATUS.md`
4. `docs/CODEX_HANDOFF.md`
5. `docs/ADR/0025-codex-only-mcp-3d-runtime.md`
6. `docs/RESET_MIGRATION_PLAN.md`
7. `docs/CODEX_EXECUTION_PLAN.md`
8. `docs/CODEX_TASK_INDEX.md`
9. `docs/AUTHORITATIVE_STATE.md`
10. 本文件
11. 当前任务相关专项合同和测试文档。

如果任一文件冲突，停止实现并按文档地图解决权威，不自行折中两条产品路线。

## 3. 任务启动模板

Luna 在动文件前必须记录：

```text
Task ID:
Dependency status:
Base commit / branch:
git status summary:
User dirty files in owned paths:
Owned paths:
Forbidden paths:
Baseline commands and exit codes:
Exit gates:
Destructive actions:
Required user approval:
```

未列明 owned paths 或退出 Gate，不得开始。

## 4. MCP001 特别协议（恢复门已通过）

MCP001 是破坏性硬切，当前分支已完成授权、归档和删除：

- 创建 `codex/forgecad-mcp-reset` 分支；
- 创建并验证可恢复归档；
- 按清单删除旧代码和文档。

后续任务仍按固定顺序：

1. 记录 base/status；
2. 生成 tracked diff、untracked archive、Library/DB/CAS backup 和 hash manifest；
3. 在临时目录完成恢复试验；
4. 只在明确授权的新 reset 分支做删除；
5. 同一任务添加 contracts/core/store/runtime/mcp/worker/viewer 最小骨架；
6. 重写构建、CI、integrity/docs gates；
7. 运行全 Gate；
8. 用户数据 hash 不变；
9. 更新状态和 handoff。

删除后仓库可以“功能暂不可用”，但必须可编译且 UI 诚实显示迁移中。不能保留旧按钮作兼容 fallback。

## 4.1 MCP002 当前执行结果

`FGC-MCP002` 已完成其退出条件：首批 contracts/Rust records、Runtime V1 migration、CAS、SQLite 单写者、事务/重启/备份恢复和 authenticated local IPC 已有 focused evidence。该历史阶段的下一项是 `FGC-MCP003`；当前执行请以 4.2 的本地实现和未完成宿主门为准，不得提前实现几何、材质或恢复旧 Provider。

## 4.2 MCP003 当前执行结果

`FGC-MCP003` 已完成本地 protocol adapter 子门，并完成一次认证 Codex CLI 真实只读模型回合及一次现代协议拒绝回合：MCP `2025-11-25` canonical 与 Codex `2025-06-18` compatibility 生命周期、只读 tools/resources、URI allowlist、typed unavailable errors、Runtime contract mismatch fail closed、原始 stdio probe、三个 Codex 配置基线、`capabilities_get`/`selection_get` CLI E2E 和 `2026-07-28` 明确不支持均已写入当前工作树。可用打包二进制时可运行 `FORGECAD_MCP_COMMAND=/path/to/forgecad-mcp npm run mcp003:stdio`；这只证明 stdio 传输层，不证明三宿主。MCP `2026-07-28` 无握手模式不是当前实现，不能通过环境变量假装兼容。Desktop/IDE 和官方 conformance 仍是 `NOT_RUN`，由 `docs/evidence/mcp003/host-matrix.json` 与 `docs/evidence/mcp003/codex-cli-e2e.json` 明确记录；Luna 不得把 CLI 单宿主 PASS、本地 Rust/静态检查、stdio probe 或无模型回合 app-server 诊断改写为三宿主通过，也不得领取 MCP004。

## 5. 实施纪律

- 使用最小原子变更，不跨到下一 Task；
- 修改前检查 dirty diff，保留用户工作；
- 文件编辑用 patch，机械格式化可用官方 formatter；
- 不 commit/push/merge，除非用户明确授权对应动作；
- 不把密钥、绝对路径、prompt、原图字节或付费调用写入测试/evidence；
- 不下载/执行 GitHub 项目、模型权重或外部脚本，除非当前任务和采用清单已批准；
- 不使用旧 Provider/Agent 来“临时打通”E2E；
- 不用任意 Blender Python、shell 或 JavaScript 生成产品几何；
- 新公开合同先 Schema、生成类型、validator、negative tests，再 runtime/tool/UI；
- 永久写功能必须先实现失败、幂等、stale base、取消和重启测试。

## 6. 外部项目与 Skill

需要 GitHub/Blender 参考时先读 `EXTERNAL_PROJECT_ADOPTION.md` 和 `SKILL_PACKAGE_STANDARD.md`。研究输出必须分为：算法思想、代码、资产、交互、许可证、供应链、可执行权限和 Benchmark。

不能整套 clone 进生产树。候选项目先在隔离研究目录固定 commit，只读审 LICENSE/NOTICE/dependencies；采用需 receipt、SBOM、安全/资源测试、平台包和退出方案。Skill P0 只允许声明式 Bundle；签名不替代沙箱或质量门。

## 7. 验证与状态

每项命令记录准确 exit code：

```text
PASS      命令在当前工作树通过
FAIL      命令运行并失败
BLOCKED   外部权限/环境/前置条件阻断
NOT_RUN   本轮未运行
```

局部测试绿色不代表 aggregate、packaged、真实 Codex、附件、GPU 或真人 Gate。能力标签严格用 `已实现/部分实现/目标设计/迁移中/superseded/blocked`。

任务只有所有 exit gates 满足时改 `done`；否则保持 `in_progress` 或按明确依赖 `blocked`。不得因上下文或预算接近结束而标完成。

## 8. 每轮 Handoff

更新 `CODEX_HANDOFF.md`、`CODEX_TASK_INDEX.md`、`DOCUMENTATION_STATUS.md`、能力矩阵和相关用户/专项文档。写清：实际代码、测试、真实运行、packaged、真人证据分别是什么；哪些没跑；为什么；下一唯一任务。

## 9. 禁止的“捷径”

- 把旧 `cad-workbench` 改名为 Viewer；
- 把 App Server 改名为 Runtime但保留 Agent/Provider/Thread/Turn；
- 让 MCP 直接写 SQLite 或调用 Python FastAPI；
- 用 `client_name` 判断安全身份；
- 假设 Codex 附件自动传入 MCP；
- 让 Viewer 截图成为 quality truth；
- 用材质/纹理堆砌掩盖轮廓/比例失败；
- 把 Skill 文档或签名当成质量证明；
- 为了通过旧 gate 恢复 Provider/U004/legacy code；
- 自动删除旧 Library 或用户资产。

## 10. Goal 完成语句

只可使用：

```text
FGC-MCPxxx completed: all listed exit gates passed on <commit/worktree>.
```

若未完成：

```text
FGC-MCPxxx not complete: <PASS/FAIL/BLOCKED/NOT_RUN evidence>; next safe action is <one action>.
```
