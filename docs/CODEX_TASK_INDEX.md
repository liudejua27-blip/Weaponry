# ForgeCAD 当前原子任务索引

版本：2026-08-08
状态：唯一任务状态表

## 1. 状态规则

状态只允许：`ready`、`in_progress`、`blocked`、`done`、`superseded`。同一时刻最多一个 `in_progress`。依赖未完成时只能 `blocked`，不能提前领取。

任务 ID 格式为 `FGC-MCP000`–`FGC-MCP013`。旧 U/C/K/F/E/VP 任务全部退出当前执行链。

## 2. 当前链

| Task ID | 状态 | 依赖 | 原子结果 |
|---|---|---|---|
| FGC-MCP000 | done | 无 | ADR-0025、权威文档重置、删除/迁移/升级清单和 Luna 任务链；文档/完整性/安全/密钥/许可证/diff Gate 通过 |
| FGC-MCP001 | done | MCP000 + 用户 reset 授权 | 可恢复快照；整组删除旧产品；最小 Viewer/Runtime/contracts 可编译骨架 |
| FGC-MCP002 | done | MCP001 | Contracts、SQLite V1/CAS、Runtime 单写者与 authenticated IPC |
| FGC-MCP003 | in_progress | MCP002 | Codex-only MCP stdio 只读 tools/resources 与三宿主连接 |
| FGC-MCP004 | blocked | MCP003 | 候选、Job、审批、confirm/reject/restore/export 事务 |
| FGC-MCP005 | blocked | MCP004 | 真实 Codex 文本/附件 → CAS → candidate → Viewer → version E2E |
| FGC-MCP006 | blocked | MCP005 | Skill Bundle V2、Registry、Benchmark、license/SBOM/provenance/signature |
| FGC-MCP007 | blocked | MCP006 | 通用几何、轮廓比例、Assembly/Part/source-map、局部修改 |
| FGC-MCP008 | blocked | MCP007 | UV/tangent、PBR、纹理、材质和 Appearance Compiler |
| FGC-MCP009 | blocked | MCP008 | headless fixed views/AOV、参考比较和 Quality Compiler |
| FGC-MCP010 | blocked | MCP009 | undo/redo、不可变版本、回退、爆炸图和 export binding |
| FGC-MCP011 | blocked | MCP010 | Job 事件、取消、崩溃恢复、并发、配额和性能 |
| FGC-MCP012 | blocked | MCP011 | 外部项目治理与首批 first-party signed Skills |
| FGC-MCP013 | blocked | MCP012 | packaged Codex E2E、升级/回滚、跨类别真人质量门 |

`MCP001` 的 reset 授权、分支和恢复验证已完成；`MCP002` 的 contracts/Store/Runtime focused evidence 和 gates 已通过并标为 `done`。`MCP003` 当前是唯一 `in_progress`，未完成其真实宿主/官方 conformance 退出门前，不得领取 MCP004。

## 3. MCP000 验收

Owned paths：`AGENTS.md`、`README.md`、`docs/**`、仅必要的文档 Gate 脚本。
禁止：修改产品代码、删除旧用户数据、继续旧 Provider/U004 实施。

退出条件：

- ADR-0025 accepted，旧 ADR/任务明确 superseded；
- 文档完整列出 delete/rewrite/migrate/upgrade/new；
- MCP、Codex、Compiler、Viewer、Skill、external adoption 合同存在；
- Luna 指南只允许从 MCP001 开始；
- 用户指南明确当前新能力不可用；
- docs walkthrough、repository integrity、safety scope、secrets、license/SBOM、`git diff --check` 有结果；
- handoff 记录工作树、命令、失败和未运行。

## 4. MCP001 验收

Owned paths：删除清单全域、新 workspace skeleton、根构建/CI/gates。
强制步骤：先恢复证明，后删除；删除与可编译骨架同任务。

退出条件：

- `DeepSeek|Qwen|DashScope|ProviderRegistry|ApiFirst|ActionLoop|Thread/Turn/Item` 不在 production source/contracts；
- 无旧 `cad-workbench`、App Server/Protocol、`apps/agent`、FastAPI/8000、Concept/Weapon/ModuleGraph 产品路径；
- 新 contracts/core/store/runtime/mcp/worker/viewer 骨架可编译；
- 旧 Library/DB/CAS hash 不变且备份可恢复；
- repository integrity 只检查新架构；
- docs 与用户能力仍标记迁移中。

## 5. 后续任务最小证据

每个任务至少提交：

- `docs/evidence/mcpXXX/manifest.json`：commit/worktree、环境、命令、exit code、artifact hashes；
- focused unit/integration/adversarial Gate；
- crash/restart/idempotency/denied path（适用时）；
- 当前/目标能力差异；
- 未运行与 blocker；
- 文档状态、能力矩阵、handoff 同步。

视觉任务还必须提交原始参考、RenderSet/AOV、QualityReport、GLB/readback、Viewer screenshot 和真人评分表；不得只提交 Markdown 结论。

## 5.1 MCP002 验收

Owned paths：`forgecad-contracts`、`forgecad-core` canonical hash、`forgecad-store`/CAS、`forgecad-runtime`/IPC、Runtime V1 migration、相关 gates 和 evidence。

退出条件：

- Project/Candidate/Version/Snapshot/Job/Event/Audit/CAS Schema、Rust records 和 manifest 无漂移检查通过；
- 新 Runtime V1 migration 在事务中创建并校验 schema version；已有旧库在没有 `schema_meta` 时 fail closed；
- 单 writer lease 支持 heartbeat、TTL crash recovery，并拒绝两个活动 writer；
- CAS 使用 SHA-256、临时文件、fsync、原子 rename，容量超限、缺失、篡改和 hash/metadata mismatch 均 fail closed；
- 数据库重启、事务约束失败、备份/恢复和 CAS 读取有 focused evidence；
- Unix authenticated IPC 使用 0600 socket、每次启动生成且仅进程内持有的 token 和常量时间校验，错误 token 被拒绝；MCP 不持有 SQLite handle；
- 当前能力仍明确为诊断级，几何/Appearance/Render/Quality/永久写入不提前开放。

## 5.2 MCP003 验收（当前 in_progress）

Owned paths：`forgecad-mcp` stdio protocol loop、Runtime read projections、contracts resource/selection records、Codex config baselines、`scripts/check_mcp003_*.py` 和 `docs/evidence/mcp003/**`。

已完成的本地退出子门：

- MCP `2025-11-25` canonical + Codex `2025-06-18` compatibility initialize 参数/响应、`notifications/initialized`、tools/resources methods 和 deterministic annotations snapshot；
- `resources/list`、`resources/read`、resource templates、capabilities/project/snapshot/selection/candidate/job/version JSON projections；
- 原始 stdio probe 覆盖 initialize、initialized、tools/list、resources/list、resources/read 和不兼容协议的 fail-closed 响应；
- 原始 stdio probe 另覆盖 Codex `2025-06-18` compatibility 和 `2026-07-28` `server/discover` 明确不支持；
- Rust focused tests 还覆盖缺失握手字段、失败会话锁定、重复 initialize、未初始化/未知工具和非法资源 URI；
- URI/opaque-ID/response-size allowlist、Runtime contract mismatch 和 unsupported protocol fail closed；
- Desktop/CLI/IDE 的无 secret、无绝对路径 `config.toml` 基线与静态检查。

仍未完成：

- 官方 MCP conformance runner；
- Codex Desktop、Codex IDE 的真实发现、连接、只读 tools/resources 和版本不兼容宿主 E2E；Codex CLI 认证只读模型回合与现代协议拒绝已 PASS，但三宿主与官方 conformance 门仍未完成；
- 以上证据完成前，MCP004 继续 `blocked`，不得把本地 protocol adapter PASS 写成三宿主 PASS。

证据：`docs/evidence/mcp003/protocol-snapshot.json`、`host-matrix.json`、`manifest.json`。

## 6. Superseded 任务

`FGC-U001`–`U005`、`U004A`–`U004F`、所有 U004 子切片、VP/E/C/K/F/R/M/S/D/T/B/P/G/A/V/L 历史任务均为 `superseded` 或历史完成，不再是当前依赖。其确定性算法只有迁移并通过新合同 Gate 后才计入 MCP 任务。

当前唯一 `in_progress` task（需保持一次一个任务）：

```text
FGC-MCP003
```
