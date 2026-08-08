# Codex 与 ForgeCAD 集成

版本：2026-08-08
状态：部分实现；MCP003 只读适配器、配置基线和认证 CLI 只读回合已通过，2026-08-08 Computer Use/Browser 宿主探测未获得可操作 Desktop/IDE，附件与打包 E2E 未实现

## 1. 用户体验

用户不在 ForgeCAD 内聊天，也不在 ForgeCAD 内上传参考或配置模型。完整流程是：

1. 用户打开 Codex，说明要设计的对象并上传有权使用的图片；
2. Codex 读取 `forgecad` 能力、项目和当前 Viewer 选择；
3. Codex 将图片导入 ForgeCAD CAS，形成 `ReferenceEvidence`；
4. Codex 调用内置 Skills 形成 `SubjectProfile`、`RepresentationPlan` 和 typed design candidate；
5. Runtime 编译几何、外观、渲染和质量证据；
6. ForgeCAD Viewer 自动显示候选、固定视图、质量问题和部件树；
7. 用户在 Codex 里提出局部修改，或在 Viewer 选择部件后回到 Codex 描述修改；
8. Codex 准备 typed change，再次编译和比较；
9. 只有用户在 Codex 中批准，Runtime 才确认不可变版本；
10. 用户可要求恢复历史、查看爆炸图或导出，仍经 prepare/approval/confirm。

ForgeCAD 单独启动时只显示 Viewer 和连接诊断，不提供“生成”假入口。

## 2. P0 支持矩阵

| Codex 宿主 | 对话 | MCP | 本地附件 | 写审批 | P0 发布要求 |
|---|---:|---:|---:|---:|---|
| Codex Desktop | 目标 | 适配器已实现；宿主未运行 | MCP005 必须实测 | MCP004+ 必须实测 | 必过 |
| Codex CLI | 目标 | 适配器与认证只读回合 PASS | MCP005 必须实测 | MCP004+ 必须实测 | 必过 |
| Codex IDE | 目标 | 适配器已实现；宿主未运行 | MCP005 必须实测 | MCP004+ 必须实测 | 必过 |
| ChatGPT Web | 不承诺 | 不承诺 | 不承诺 | 不承诺 | 不在 P0 |
| 其他 MCP Client | 不支持 | 不验收 | 不验收 | 不验收 | 未来 ADR |

“Codex-only”是发布、配置和测试范围，不是可伪造 client name 的安全判断。

MCP003 的本地证据在 `docs/evidence/mcp003/`：协议适配器、resources/read、只读工具和版本不兼容 fail-closed 已通过；认证 Codex CLI 真实回合已完成 `capabilities_get`、`selection_get`，并在 `2026-07-28` 环境下明确拒绝且不降级；Desktop/IDE discovery/connection 本轮为 `BLOCKED`，read-only E2E 为 `NOT_RUN`，不能把单宿主证据写成三宿主完成。

2026-08-08 宿主验收记录：Computer Use 对 `com.openai.codex` 的只读状态请求被主机安全边界拒绝；Codex in-app Browser 连接成功但没有当前或用户标签页。未点击、输入、上传、改配置或发起宿主 MCP 调用，故 Desktop/IDE 的 discovery/connection 记为 `BLOCKED`，read-only E2E 记为 `NOT_RUN`，而不是 PASS。

### 2.1 MCP003 宿主验收 Runbook

这一步必须在用户拥有的 Codex 宿主中执行；仓库脚本和官方 SDK 只能证明协议互操作，不能替代宿主连接。

前置准备：

1. 使用当前分支构建 MCP 二进制到临时目录，不覆盖用户安装：
   `CARGO_TARGET_DIR=/tmp/forgecad-mcp003-cargo-target script/with_rust_toolchain.sh cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-mcp --offline`
2. 先运行 `FORGECAD_MCP_COMMAND=/tmp/forgecad-mcp003-cargo-target/debug/forgecad-mcp npm run mcp003:stdio`；失败时先修复 MCP003，不得进入宿主判定。
3. 使用 `config/codex/desktop.toml`、`cli.toml` 或 `ide.toml` 的字段。配置只放 `forgecad-mcp`、`serve --stdio` 和两个环境变量名；不复制 token、API key、绝对路径或真实用户附件。

认证 CLI 的可重复探测（仅在用户明确执行时运行）：

```bash
FORGECAD_MCP_COMMAND=/tmp/forgecad-mcp003-cargo-target/debug/forgecad-mcp \
  python3 scripts/probe_mcp003_codex_cli.py --execute --mode read-only
FORGECAD_MCP_COMMAND=/tmp/forgecad-mcp003-cargo-target/debug/forgecad-mcp \
  python3 scripts/probe_mcp003_codex_cli.py --execute --mode version-mismatch
```

脚本使用隔离临时目录、`--ephemeral`、`--ignore-user-config` 和只读沙箱；默认不启动 Codex、不联网、不写入。只读模式要求两个工具各成功一次（完成顺序不作为合同），版本模式要求无工具调用且宿主报告 fail-closed；它是验收辅助，不替代 Desktop/IDE 或官方 conformance，也不加入发布 Gate。

每个宿主都必须按以下只读序列执行：

1. 发现 `forgecad` Server，并记录 Server 名称和版本；
2. `initialize` 使用宿主实际发送的 `protocolVersion`：当前 Codex stdio 默认观测为 `2025-06-18`，官方 2025-era 客户端可使用 `2025-11-25`；两者都必须在响应中原样协商；空 `capabilities` 和非空 `clientInfo` 仍是必需字段；
3. `capabilities_get`、`project_list`、`resources/list`；
4. `resources/read` 读取 `forgecad://capabilities`；
5. `tools/call` 调用 `selection_get` 和 `version_list`，确认没有创建项目、Job 或版本；
6. 记录宿主 transcript 或截图、工具结果、Runtime 日志中的 request/response hash；不得记录 token、附件绝对路径或模型私有内容。

预期结果：初始化成功；工具数量为 14 且全部 `readOnlyHint=true`；至少发现 `forgecad://capabilities`；capabilities MIME 为 `application/json`；`selection_get.available=false`；没有写事务。任何异常都记为 `connection=BLOCKED` 或 `read_only_e2e=BLOCKED`，不能降级为 PASS。

宿主专用动作：

- Desktop：在 Codex MCP 设置中加载 ForgeCAD Server，重启/重新打开新线程后执行上述序列；不要用本仓库自动化读取 Codex Desktop UI。
- CLI：用临时 `-c` 覆盖或用户明确配置加载 `config/codex/cli.toml`，在新的 Codex CLI 会话中执行上述序列；`codex mcp get` 只证明配置发现，不证明连接。
- IDE：在 Codex IDE 的 MCP 配置中加载 `config/codex/ide.toml`，重启扩展/窗口后执行上述序列；记录扩展版本和宿主日志。

版本不兼容测试由 `mcp003:stdio` 单独执行：发送 `protocolVersion=0.0.0`，必须返回 `CONTRACT_VERSION_UNSUPPORTED` 并锁定会话；配置 `CODEX_MCP_PROTOCOL_VERSION=2026-07-28` 也必须明确报告现代协议尚未支持，不能静默降级。宿主不能为了通过测试而伪造客户端名称；`clientInfo.name` 不是认证。

完成后只把真实证据回填到 `docs/evidence/mcp003/host-matrix.json`，每行必须包含：

```json
{
  "discovery": "PASS",
  "connection": "PASS",
  "read_only_e2e": "PASS",
  "evidence": "host transcript/screenshot and bounded response receipt",
  "checked_at": "YYYY-MM-DD"
}
```

在三行宿主都通过前，`FGC-MCP003` 保持 `in_progress`，`FGC-MCP004` 保持 `blocked`。

## 3. Codex instructions

随 Server 提供的 instructions 必须要求 Codex：

- 先读 `capabilities_get`、`project_list` 和需要的 snapshot/resource；
- 不猜测不可用能力；
- 对含糊对象、缺失视图、尺寸或材质先向用户澄清；
- 只提交公开 typed Schema，不提交任意脚本；
- 长任务通过 job 轮询并允许用户取消；
- 读取结构、渲染和质量证据后再建议确认；
- 将硬门失败直接告诉用户，不用语言评价覆盖；
- 任何永久写入、恢复、安装 Skill 和导出都使用 Codex write approval；
- 不把 Viewer 截图、自然语言或单一 GLB 可打开等同高质量通过。

## 4. Viewer 联动

Viewer 的相机、选择、隔离和临时爆炸距离是 ephemeral UI state。MCP003 的 `selection_get` 和 selection resource 会诚实返回 `available=false`，直到 MCP010 把 Viewer 选择接入单一版本真值；不能把空选择投影成稳定 Part ID。Viewer 不直接发送 prompt，也不拥有会话。

候选完成时 Runtime 发布本地事件，Viewer 刷新 read model；Codex 无需保证 Viewer 打开。Viewer 关闭时所有 compile/render/evaluate、版本和导出语义保持不变。

## 5. 安装与更新

安装器交付同版本、同签名的 Runtime、MCP、Viewer 和 workers，并为用户生成 Codex MCP 配置。更新流程先验证签名、合同兼容和数据库备份，再原子切换；失败回滚整套二进制，不能混用不同合同版本。

不在配置中保存 OpenAI API Key。Codex 的身份、订阅和模型调用由 Codex 自身管理，ForgeCAD 不读取或复制这些凭据。

## 6. 真实验收脚本

最终 packaged Gate 必须由普通用户路径完成：

1. 新安装且无开发环境变量；
2. 在 Codex 上传单图和多视图参考；
3. Codex 发现 Server，成功导入原始字节并读取 hash；
4. 创建项目和候选，Viewer 展示同一 candidate hash；
5. 运行几何、UV/PBR、固定视图和参考比较；
6. Viewer 选择一个 Part，Codex 对该稳定 ID 做局部修改；
7. 用户拒绝一次，证明无版本写入；再批准一次，证明只创建一个子版本；
8. 重启 Runtime/Viewer/Codex 后读回同一版本和工件；
9. 生成爆炸图、恢复历史为新版本并导出；
10. 收集 tool transcript、job events、approval receipt、quality report、render set、GLB/readback、export manifest 和真人评分。

任何本地 fake、fixture、离线 Provider、手工复制图片或开发脚本替代都不能通过这道门。
