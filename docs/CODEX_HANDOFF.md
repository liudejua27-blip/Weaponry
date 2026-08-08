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
| `python3 scripts/check_mcp003_hosts.py` | PASS：基线无 secret/绝对路径；CLI 配置发现与认证只读回合 PASS；Desktop/IDE discovery/connection 为 BLOCKED，read-only E2E 未运行 |
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
| Codex Desktop/IDE UI probe | BLOCKED：本轮 Computer Use 访问 `com.openai.codex` 被主机安全边界拒绝；Codex in-app Browser 连接成功但没有可控标签页。未执行任何 UI、配置、上传或 MCP 宿主操作，Desktop/IDE discovery/connection 已按阻断记录，见 `host-matrix.json` 的 `computer_use_host_probe` |
| Codex Desktop/IDE、官方 conformance、附件、packaged Viewer、视觉门 | 未运行；CLI 真实回合已在上方 PASS |

## 下一步唯一动作

`FGC-MCP003` 的本地实现、原始 stdio 探测和认证 Codex CLI 只读回合已完成，但退出条件尚未满足：官方 conformance 尚未运行，Desktop/IDE 真实连接也未运行。本轮已尝试电脑只读探测和浏览器只读检查，但均未获得可操作宿主，不能据此推断 PASS。下一唯一动作仍是在用户拥有且已解锁的宿主中按 `docs/evidence/mcp003/host-matrix.json` 执行 Desktop/IDE 发现、连接、只读工具和版本不兼容 smoke；不得把 CLI 单宿主 PASS、stdio 探测、本地适配器或 app-server 诊断 PASS 改写成三宿主 E2E PASS，也不得恢复旧测试或 Provider 路径。
