# ForgeCAD MCP Runtime 合同

版本：2026-08-08
状态：部分实现；MCP003 已实现稳定协议协商、只读 tools/resources、配置基线和认证 Codex CLI 只读回合，Desktop/IDE 与官方 conformance 仍未运行
P0 客户端：Codex Desktop、Codex CLI、Codex IDE

## 1. 合同目标

`forgecad-mcp` 将模型无关的 ForgeCAD Runtime 暴露给 Codex。它不包含 Agent、聊天、图片理解、模型 SDK、Provider、项目数据库或几何算法。所有工具输入先经公开 JSON Schema 验证，再调用本机 Runtime；所有输出都包含稳定 ID、Schema 版本、hash、lineage、能力状态和可恢复错误。

P0 使用 MCP `stdio`。Streamable HTTP、远程多租户、OAuth 和通用 MCP Client 均不在范围内。MCP Tasks/Skills 等可选协议扩展不能成为 P0 前置条件；长任务使用普通工具返回持久 `RuntimeJob`。

## 2. 进程和信任边界

```text
Codex ──stdio── forgecad-mcp ──local authenticated IPC── forgecad-runtime
                                                     ├── SQLite V1
                                                     ├── CAS
                                                     └── restricted workers
```

- `forgecad-mcp` 在配置 `FORGECAD_RUNTIME_SOCKET`/`FORGECAD_RUNTIME_TOKEN` 时通过 Runtime 每次启动生成、仅在进程内持有的会话令牌连接；令牌不进入日志、资源或工具结果。无配置路径仅用于 ephemeral diagnostic smoke，打包发布必须使用 Runtime IPC；
- MCP 进程无数据库写权限，无任意项目文件系统权限，不监听 TCP 端口；
- Runtime 校验 project scope、base revision、idempotency key、candidate hash、approval receipt 和 tool capability；
- Worker 只接受受限内部协议，不接受 Codex 生成的 Python、JavaScript、shell、URL 或绝对文件路径；
- 工具失败时返回 typed error，不回退 legacy HTTP、Provider 或第二状态写者。

## 3. Server 信息与能力协商

Server 名：`forgecad`。Server instructions 的前 512 字符必须自包含地说明：这是本地 3D Runtime；先读取能力和项目；永久写入需候选与用户批准；长任务返回 job；禁止发送任意代码和未授权路径。

MCP003 使用 2025-era 的有状态 stdio 生命周期：`2025-11-25` 是 ForgeCAD 的规范版本，同时明确兼容 Codex 当前 stdio 默认发送的 `2025-06-18`。初始化必须包含 `protocolVersion`、`capabilities`、`clientInfo`；只接受这两个版本，并在响应中返回实际协商的版本。不匹配的版本、缺失参数或 Runtime 合同会 fail closed。初始化后 Codex 必须先调用 `capabilities_get`。返回：

- `runtime_version`、`contract_versions`、`tool_manifest_hash`；
- Viewer/Runtime/Worker/Skill 状态；
- 支持的几何、材质、纹理、渲染、质量、导入/导出格式；
- 图片导入模式及尺寸/MIME 上限；
- 每个工具 `available | unavailable | degraded` 与 limitation；
- 当前项目授权范围和写审批策略。

若 Server/Runtime/合同版本不兼容，所有写工具 fail closed；只允许诊断和导出备份。

## 4. 资源

| URI | 内容 | 约束 |
|---|---|---|
| `forgecad://capabilities` | 当前能力快照 | 无目标能力伪装 |
| `forgecad://projects/{project_id}/snapshot` | `ActiveDesignSnapshot` | 单一当前投影 |
| `forgecad://projects/{project_id}/selection` | Viewer 当前临时选择 | 非版本真值 |
| `forgecad://candidates/{candidate_id}` | 候选、readback、quality 摘要 | hash-bound |
| `forgecad://jobs/{job_id}` | Job 与最近事件 | 可重启读取 |
| `forgecad://versions/{version_id}` | 不可变版本和工件 manifest | 只读 |
| `forgecad://renders/{render_set_id}/{pass}` | 固定视图/AOV 图像 | MCP003 仅发现模板；Render Compiler 未启用 |
| `forgecad://skills/{skill_id}/{version}` | 已安装 Bundle 清单 | MCP003 仅发现模板；Registry 未启用 |
| `forgecad://artifacts/{artifact_id}` | hash-bound 工件元数据 | MCP003 仅发现模板；binary readback 未启用 |

MCP003 当前已实现 capabilities、项目 snapshot/selection、candidate、job、version 的 JSON projection 和对应 resource templates；renders、skills、artifact binary link 仍由后续能力门启用，不会出现在 `resources/list` 中。资源 URI 只接受 `forgecad://` 和受限 opaque ID，不接受文件路径、URL、查询串或 `..`。

大二进制不内联到日志或事件；通过 MCP resource link 或受限 blob 读取，并声明 MIME、字节数和 SHA-256。

## 5. 工具目录

### 5.1 只读工具

- `capabilities_get`
- `project_list`
- `project_get`
- `snapshot_get`
- `selection_get`
- `candidate_get`
- `job_get`
- `job_events_read`
- `version_list`
- `version_diff`
- `skill_list`
- `skill_get`
- `quality_get`
- `artifact_readback_get`

MCP003 的工具清单固定排序并声明 `readOnlyHint=true`、`destructiveHint=false`、`idempotentHint=true`、`openWorldHint=false`。尚未实现的 quality/Skill/diff/artifact 工具仍可被发现，但返回 `CAPABILITY_UNAVAILABLE` typed error；不得以自然语言伪装成功。

这些工具必须声明 read-only annotation，且不能以“读取”为名创建项目、下载网络资产、运行编译或改变 GC 生命周期。

### 5.2 候选/任务工具

- `project_create_prepare`
- `reference_import`
- `design_prepare`
- `candidate_compile`
- `candidate_render`
- `candidate_evaluate`
- `change_prepare`
- `exploded_view_prepare`
- `restore_prepare`
- `export_prepare`
- `job_cancel`
- `visual_review_submit`

它们可写临时 Job/CAS/candidate 状态，但不能创建永久资产版本。由于会读取文件、占用计算或创建临时工件，Codex 配置中按 write 工具处理。

### 5.3 永久写工具

- `project_create_confirm`
- `candidate_confirm`
- `candidate_reject`
- `restore_confirm`
- `export_confirm`
- `skill_install_confirm`
- `skill_disable_confirm`

永久工具必须绑定：

```json
{
  "project_id": "...",
  "base_version_id": "...",
  "prepared_object_id": "...",
  "prepared_object_sha256": "...",
  "quality_report_id": "...",
  "approval_receipt_id": "...",
  "idempotency_key": "..."
}
```

Runtime 必须确认审批未过期、范围和 hash 完全一致、基线未漂移、硬质量门通过。确认结果是不可变版本；同一幂等键重复调用返回同一结果。

## 6. 参考图片导入

`reference_import` 只接受以下二选一来源：

1. `inline_content`：受合同限制的 MIME、尺寸和字节数；
2. `codex_local_file`：Codex 提供的本地附件路径，但必须位于启动时显式授权的 attachment roots 或 OS 单文件授权内。

路径处理顺序固定：canonicalize → 拒绝 symlink/目录/设备文件 → 检查 root → MIME sniff → size/dimension/decompression-bomb 检查 → 计算 hash → 复制到 CAS → 丢弃原始路径。日志和永久对象不得保存用户名或绝对路径。

P0 Gate 必须分别在 Codex Desktop、CLI 和 IDE 上证明实际附件字节能进入 CAS；客户端只让 Codex“看见图片”不算通过。若某客户端不能传附件，能力快照必须明确 `unavailable`，不得静默用语言描述替代原图。

## 7. Job 合同

预计超过 10 秒的操作必须在 2 秒内返回：

```json
{
  "schema_version": "RuntimeJob@1",
  "job_id": "job_...",
  "kind": "candidate_compile",
  "state": "queued",
  "project_id": "...",
  "request_sha256": "...",
  "created_at": "...",
  "poll_after_ms": 1000,
  "cancel_supported": true
}
```

状态：`queued | running | waiting_for_input | succeeded | failed | cancelled`。事件为单调序列、可分页和重放；事件只引用 CAS，不含 prompt、图片字节、密钥和绝对路径。Runtime 重启后，终态可读取；非终态只能按已提交 checkpoint 续接，否则转为 typed failure，不能假装继续。

## 8. 错误合同

至少支持：

- `CAPABILITY_UNAVAILABLE`
- `CONTRACT_VERSION_UNSUPPORTED`
- `PROJECT_SCOPE_DENIED`
- `REFERENCE_TRANSFER_UNAVAILABLE`
- `REFERENCE_REJECTED`
- `STALE_BASE_VERSION`
- `CANDIDATE_HASH_MISMATCH`
- `APPROVAL_REQUIRED`
- `APPROVAL_EXPIRED`
- `QUALITY_HARD_GATE_FAILED`
- `SKILL_UNTRUSTED`
- `WORKER_BUDGET_EXCEEDED`
- `JOB_CANCELLED`
- `RUNTIME_RECOVERY_REQUIRED`

错误包含机器可读 code、safe message、retryable、next action 和 evidence IDs；不得返回 stack trace、原始请求、密钥或本机绝对路径。

## 9. Codex 配置基线

开发期基线位于 `config/codex/desktop.toml`、`config/codex/cli.toml`、`config/codex/ide.toml`。基线不设置 `CODEX_MCP_PROTOCOL_VERSION`，因此使用 Codex 的 2025-era 默认兼容路径：

```toml
[mcp_servers.forgecad]
command = "forgecad-mcp"
args = ["serve", "--stdio"]
env_vars = ["FORGECAD_RUNTIME_SOCKET", "FORGECAD_RUNTIME_TOKEN"]
startup_timeout_sec = 20
tool_timeout_sec = 60
required = true
default_tools_approval_mode = "writes"
```

项目级 `.codex/config.toml` 只允许固定签名二进制和相对项目授权；不得提交用户目录、secret、Library 路径或开发机绝对路径。发布安装器负责把 `forgecad-mcp` 解析到签名路径，并展示将启用的工具和写审批策略；基线只提交环境变量名，不提交令牌值。

## 10. MCP002 已通过的合同 Gate

- 首批 Project/Candidate/Version/Snapshot/Job/Event/Audit/CAS Schema、Rust records 和 manifest 无漂移检查；
- Runtime V1 migration、legacy database rejection、WAL/foreign keys/busy timeout、事务回滚、重启和 backup/restore；
- CAS SHA-256、容量限制、临时文件 + fsync + 原子 rename、missing/corrupt/hash mismatch；
- writer lease 并发拒绝、heartbeat、TTL recovery；
- Unix socket 0600、token hash + constant-time comparison、错误 token fail closed；
- MCP crate 不依赖 SQLite，IPC read dispatch 只返回结构化 Runtime projection。

## 11. MCP003 已完成的本地合同 Gate

- `docs/evidence/mcp003/protocol-snapshot.json` 固定 MCP `2025-11-25`、Codex `2025-06-18` 兼容版本、initialize 字段、method、tools、annotations、resource templates 和 1 MiB projection 上限；
- `resources/list`、`resources/read`、`resources/templates/list` 和 14 个只读工具由 Rust 单元测试及静态合同检查覆盖；
- `npm run mcp003:stdio` 可对已打包或已构建的 `forgecad-mcp` 运行无配置副作用的原始 stdio 探测：校验四个响应、14 个只读工具、能力资源和协议不兼容 fail-closed；它是传输层证据，不等于 Codex Desktop/CLI/IDE 宿主 E2E；
- 官方 `@modelcontextprotocol/sdk` 的 `StdioClientTransport` 独立探测已列出 14 个工具、1 个资源并读回 capabilities；这是协议客户端互操作证据，不是 Codex 宿主或 conformance 证据；
- Server/Runtime contract mismatch、协议版本不支持、非法 URI、非法 opaque ID 和未实现能力均 fail closed；
- Desktop/CLI/IDE 配置基线不含 secret、绝对路径或现代协议 opt-in，`docs/evidence/mcp003/host-matrix.json` 诚实记录本地适配器 PASS、CLI 认证只读回合 PASS 与 Desktop/IDE 未运行状态。

## 12. 协议版本边界与宿主诊断

MCP `2026-07-28` 是另一种协议时代：它移除了 `initialize`/`notifications/initialized` 和会话状态，使用 `server/discover`、每请求 `_meta` 与 `requestState`。ForgeCAD MCP003 不宣称支持该现代 wire mode；把它和 2025-era 状态机混在一个未标注的进程中会造成错误的安全和生命周期假设。若配置了 `CODEX_MCP_PROTOCOL_VERSION=2026-07-28`，MCP003 应明确失败，而不是静默降级。待 Codex 宿主和 ForgeCAD 分别完成现代 stdio adapter 合同后，再以独立任务引入。

本地 Codex app-server 诊断已证明当前版本会启动 MCP 连接并发送 `2025-06-18`；此前只接受 `2025-11-25` 会返回 `CONTRACT_VERSION_UNSUPPORTED`。一次真实、认证的 `codex exec` 只读模型回合随后完成了 `capabilities_get` 和 `selection_get`，未发生写事务；设置 `CODEX_MCP_PROTOCOL_VERSION=2026-07-28` 的第二次真实回合则明确返回 unsupported-protocol 且没有工具调用或静默降级。因此 MCP003 将 2025-06-18 作为显式兼容版本保留，不能把官方 SDK 使用的 2025-11-25 探针结果或 app-server 诊断单独当成三宿主证据。

## 13. 完整合同 Gate（MCP004+）

- 官方 MCP conformance 与 Codex Desktop/CLI/IDE 三客户端 smoke；
- tools/list、resources/list、Schema、annotations 和 server instructions snapshot；
- 每个工具成功、非法输入、重复请求、stale base、越权、取消、重启和超时测试；
- MCP 关闭/Viewer 关闭不破坏 Runtime；

注意：截至本版本，[官方 `@modelcontextprotocol/conformance`](https://github.com/modelcontextprotocol/conformance) 的 server 命令以 Streamable HTTP URL 为入口，而 MCP003 产品合同固定为本地 stdio。不能把一个临时 HTTP 代理的结果写成 stdio Server 已通过官方 conformance；要关闭这一项，必须另立 transport adapter 合同并分别验证，或采用官方支持 stdio 的 runner。当前证据因此诚实保留为 `NOT_RUN`，而不是伪造 PASS。
- Runtime 关闭时 MCP 明确失败且不启动 legacy sidecar；
- 任何永久版本都能回溯到请求 hash、候选、质量、审批、Skill 和工件 hash。

## 14. 版本参考

- [OpenAI Codex MCP 文档](https://developers.openai.com/codex/mcp/)和 [Codex MCP connection manager](https://github.com/openai/codex/blob/main/codex-rs/codex-mcp/src/connection_manager.rs)决定 P0 实际 Codex 配置、默认 legacy 版本和显式 modern opt-in；
- [MCP lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)和 [MCP resources](https://modelcontextprotocol.io/specification/2025-11-25/server/resources)用于协议/资源设计；
- [MCP 2026-07-28 发布说明](https://blog.modelcontextprotocol.io/posts/2026-07-28/)只用于规划未来无握手/无会话 adapter，不代表 MCP003 已实现；
- [MCP Tasks extension](https://modelcontextprotocol.io/extensions/tasks/overview)和 Skills-over-MCP 仍需按客户端能力协商，P0 不依赖它们。

MCP 规范与 Codex 已发布行为可能不同步。`FGC-MCP003` 已 pin 协议版本和配置基线，认证 CLI 只读回合已有证据，但官方 conformance runner 与 Desktop/IDE 连接仍需在用户拥有的宿主会话中运行；不得仅按本地适配器测试或无模型回合的 app-server 诊断假定三宿主可用，也不得依赖已废弃 Roots/Sampling/Logging。
