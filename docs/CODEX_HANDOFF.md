# ForgeCAD 当前交接

更新时间：2026-08-08 · 分支：`main` · 基线：`226b437c` · 任务：`FGC-MCP003 in_progress`

## 已完成

- 创建可恢复备份 `/tmp/forgecad-mcp001-20260807`，包含 tracked diff、untracked archive、Library archive、Git bundle 和 SHA-256；在临时目录验证了 patch reverse、archive 解包、`library.db` 可读和 CAS 目录存在；
- 删除旧 `apps/desktop/src` 工作台及共享桥接、Tauri 旧源、App Server/Protocol crates、Python Agent、旧 contracts/skills/migrations、旧脚本、旧 Provider/U004 文档和历史 evidence；
- 创建 `forgecad-contracts`、`forgecad-core`、`forgecad-store`、`forgecad-runtime`、`forgecad-mcp`、`forgecad-worker-protocol`、geometry/render worker skeleton；
- 创建只读 Runtime Viewer，重写 root scripts、contracts/integrity/secrets/safety/license/docs gates；
- Rust workspace、core/store/runtime tests、worker checks、MCP handshake、Viewer build 和 `npm run release:mcp002` 已通过；
- MCP002 已补齐首批 Project/Candidate/Version/Snapshot/Job/Event/Audit/CAS contracts；canonical JSON/hash、Runtime V1 migration、SQLite WAL/foreign-key/busy-timeout、事务回滚、旧库拒绝、备份恢复、CAS 原子写入/篡改检测、writer lease TTL recovery 和 authenticated Unix IPC 已通过 focused tests；
- `forgecad-mcp` 无数据库依赖；设置 `FORGECAD_RUNTIME_SOCKET`/`FORGECAD_RUNTIME_TOKEN` 时走 authenticated local IPC，未设置时只保留 ephemeral diagnostic backend；
- MCP003 已实现 MCP `2025-11-25` canonical + Codex `2025-06-18` compatibility initialize/version negotiation、`notifications/initialized`、只读 `tools/list`/`tools/call`、`resources/list`/`resources/read`/templates、URI allowlist、typed unavailable errors 和 Runtime contract mismatch fail closed；
- 增加 `config/codex/{desktop,cli,ide}.toml`，以及 `docs/evidence/mcp003/{protocol-snapshot,host-matrix,manifest}.json` 和两个静态 Gate；
- 未修改 `WushenForgeLibrary`；reset 已提交并推送到 `main`（`226b437c`）。

## 当前拥有文件

MCP001 owned：`apps/desktop/src/**`、`apps/desktop/src-tauri/**` 新 workspace、`apps/geometry-worker/**`、`apps/render-worker/**`、`migrations-runtime-v1/**`、`packages/forgecad-contracts/**`、`scripts/**` 新 Gate、根构建配置和当前 authority docs。用户未提交内容由 reset archive 保护；不要 reset/checkout/clean 或删除 `WushenForgeLibrary`。

## 验证状态

| 命令 | 状态 |
|---|---|
| `python3 scripts/check_forgecad_contracts.py` | PASS：15 schemas |
| `python3 scripts/check_mcp002_runtime.py` | PASS |
| `python3 scripts/check_mcp003_protocol.py` | PASS |
| `python3 scripts/check_mcp003_hosts.py` | PASS：基线无 secret/绝对路径；CLI 与 Desktop 配置/只读调用 PASS；IDE discovery/connection 为 BLOCKED，Desktop initialize protocol 未记录、host version mismatch 未运行 |
| `FORGECAD_MCP_COMMAND=/path/to/forgecad-mcp npm run mcp003:stdio` | PASS：4 个 stdio 响应、14 个只读工具、能力资源和协议不兼容 fail-closed；不替代宿主 E2E |
| `@modelcontextprotocol/sdk` `StdioClientTransport` probe | PASS：官方 SDK 客户端列出 14 个工具、1 个资源并读取 capabilities；协议级证据，不替代 Codex 宿主或官方 conformance |
| `npm run desktop:typecheck` | PASS |
| `npm run desktop:build` | PASS |
| `script/with_rust_toolchain.sh cargo check --workspace --offline` | PASS（Tauri workspace，离线） |
| `npm run release:mcp003` | BLOCKED：前置静态、合同、Runtime、Worker、Viewer Gate 已通过；最后的默认 Tauri 增量检查在旧 target 上长时间无输出后被安全中断（exit 130），不影响宿主矩阵结论 |
| Store/CAS focused tests | PASS：10 tests（lease、migration/restart、rollback、CAS corruption/capacity、backup/restore、concurrency、legacy rejection） |
| Runtime authenticated IPC | PASS：wrong token rejected；authorized client reads capabilities |
| MCP/worker focused runtime tests | PASS：10 个 MCP 测试覆盖 canonical/ Codex 兼容协商、现代 discovery 明确拒绝、握手缺字段、重复初始化、未知工具、非法 URI、resources/version fail-closed；geometry/quality/Skill unavailable receipt |
| Codex app-server bounded diagnostic | PASS：临时 CODEX_HOME 的 Codex Desktop/0.147.0-alpha.6.5 启动只读线程，列出 14 tools/1 resource，并通过 `mcpServer/tool/call` 调用 `selection_get`；不等同三宿主 UI/CLI/IDE E2E |
| Codex CLI real model-turn E2E | PASS：真实认证 `codex exec` 在隔离只读项目启动 thread/turn，并完成 `capabilities_get`、`selection_get` 两个只读 MCP 调用；无用户配置、项目或 Runtime 写入，见 `docs/evidence/mcp003/codex-cli-e2e.json` |
| Codex CLI modern protocol mismatch | PASS：设置 `CODEX_MCP_PROTOCOL_VERSION=2026-07-28` 后返回明确 unsupported-protocol 响应；无 MCP 工具调用、无静默降级、无副作用，见同一证据文件 |
| Codex CLI repeatable probe | PASS：`scripts/probe_mcp003_codex_cli.py` 的显式 `--execute` 只读模式和版本不兼容模式均已运行；只读工具完成顺序不作为合同，输出为脱敏 JSON receipt；脚本默认不启动 Codex且不加入发布 Gate |
| Codex Desktop user-provided transcript | PASS（只读调用）：截图和 transcript 证明 Desktop 完成 capabilities/project/resources/selection/version 读取且无写事务；initialize.protocolVersion 未记录，host version mismatch 未运行 |
| Current Codex app MCP read-only receipt | PASS：本轮重新完成 capabilities_get、project_list、resources/list、resources/read、selection_get、version_list；无写事务；自动 initialize 响应与版本不兼容结果仍未暴露，见 `docs/evidence/mcp003/codex-current-session-readonly.json` |
| Codex Desktop/IDE UI probe | BLOCKED：本轮 Computer Use 访问 `com.openai.codex` 被主机安全边界拒绝；Codex in-app Browser 连接成功但没有可控标签页。用户手动证据已单独记录，IDE 仍未运行，见 `host-matrix.json` 的 `computer_use_host_probe` |
| Codex Desktop/IDE、官方 conformance、附件、packaged Viewer、视觉门 | 未运行；CLI 真实回合已在上方 PASS |

## 下一步唯一动作

`FGC-MCP003` 的本地实现、原始 stdio 探测、认证 Codex CLI 只读回合和用户提供的 Desktop 只读工具/资源回合已完成，但退出条件尚未满足：Desktop 的原始 initialize.protocolVersion 未记录，Desktop host version mismatch 未运行，IDE 真实连接和官方 conformance 也未运行。下一唯一动作是补齐 Desktop 初始化/不兼容证据并在用户宿主中执行 IDE 序列；不得把用户截图、CLI 单宿主 PASS、stdio 探测、本地适配器或 app-server 诊断 PASS 改写成三宿主 E2E PASS，也不得恢复旧测试或 Provider 路径。
