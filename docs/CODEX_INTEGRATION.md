# Codex 与 ForgeCAD 集成

版本：2026-08-09
状态：MCP005–MCP009 MVP host golden path 已完成；当前 P0 连接为 Codex Desktop/CLI；视觉质量、Desktop write 和签名 packaged E2E 仍分层记录，签名发布移到 MCP013

2026-08-17：只读 `geometry_program_hash` 另支持 `ParametricDesignKitRequest@1`，由 Runtime 展开 housing/panel/frame、vent、joint、sensor 六类 typed macro；返回 hash-bound `GeometryProgram@2`/`source_map`，不创建 candidate/CAS/版本，也不允许任意插件或脚本进入 Runtime。该 source slice 的 structural-only 证据见 `docs/evidence/mcp010f/parametric-design-kit-v0-source-gate-20260817.json`。

## 1. 用户体验

用户不在 ForgeCAD 内聊天，也不在 ForgeCAD 内上传参考或配置模型。完整流程是：

1. 用户打开 Codex，说明要设计的对象并上传有权使用的图片；
2. Codex 先通过 `skill_get` 读取 `ponytail-preflight@0.1.0`，判断是否已有受限能力可复用并选择最小 typed action；
3. Codex 读取 `forgecad` 能力、项目和当前 Viewer 选择；
4. Codex 将图片导入 ForgeCAD CAS，形成 `ReferenceEvidence`；
5. Codex 调用内置 Skills 形成 `SubjectProfile`、`RepresentationPlan` 和 typed design candidate；
6. Runtime 编译几何、外观、渲染和质量证据；
7. ForgeCAD Viewer 自动显示候选、固定视图、质量问题和部件树；
8. 用户在 Codex 里提出局部修改，或在 Viewer 选择部件后回到 Codex 描述修改；
9. Codex 准备 typed change，再次编译和比较；
10. 只有用户在 Codex 中批准，Runtime 才确认不可变版本；
11. 用户可要求恢复历史、查看爆炸图或导出，仍经 prepare/approval/confirm。

ForgeCAD 单独启动时只显示 Viewer 和连接诊断，不提供“生成”假入口。

## 2. P0 支持矩阵

| Codex 宿主 | 当前范围 | discovery | connection | initialize.protocolVersion | read-only E2E | 协议 mismatch | 无副作用 | P0 发布要求 |
|---|---|---:|---:|---|---:|---|---:|---|
| Codex Desktop | REQUIRED | PASS | PASS | `2025-06-18`（真实握手） | PASS | `HOST_OVERRIDE_IGNORED / NOT_APPLICABLE` | PASS | 必过 |
| Codex CLI | REQUIRED | PASS | PASS | 由当前 Codex stdio 兼容路径协商 | PASS | PASS：`2026-07-28` fail-closed | PASS | 必过 |
| Codex IDE / VS Code / Cursor / Windsurf | OPTIONAL_NOT_IN_SCOPE | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 不适用 | 未要求 | 不阻塞 MCP003/MCP004 |
| ChatGPT Web | 未来/不承诺 | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 不适用 | 未要求 | 不在 P0 |
| 其他 MCP Client | FUTURE_NOT_IN_SCOPE | NOT_RUN | NOT_RUN | NOT_RUN | NOT_RUN | 不适用 | 未要求 | 未来兼容目标 |

“Codex-only”是发布、配置和测试范围，不是可伪造 client name 的安全判断。MCP003 的 REQUIRED 发布范围只有 Codex Desktop、Codex CLI 和 ForgeCAD protocol adapter；IDE 兼容代码和配置基线保留，但不安装 VS Code、不作为当前产品入口，也不作为发布 Gate。

MCP003 的真实证据在 `docs/evidence/mcp003/`：原始/SDK protocol adapter、resources/read、只读工具和版本不兼容 fail-closed 已通过；认证 Codex CLI 真实回合已完成 `capabilities_get`、`selection_get`，并在 `2026-07-28` 环境下明确拒绝、无工具调用、无静默降级和无副作用；`codex-desktop-handshake.jsonl` 和 `host-handshake.jsonl` 记录 Desktop 实际 `initialize.protocolVersion=2025-06-18` 且 ForgeCAD 返回相同值，Desktop 只读证据证明无 ForgeCAD 项目、Job、模型或版本写入；`launchctl` override 被 Desktop 忽略，因此 Desktop forced mismatch 记录为 `HOST_OVERRIDE_IGNORED / NOT_APPLICABLE`。其中 `host-handshake.jsonl` 的观测器只做透明原样转发和记录，不改写/合成请求，也不作为写入证据。IDE 未运行是已知的非 P0 范围，不是 MCP003/MCP004 阻断。

MCP004 当前在 Runtime/authenticated IPC 提供 typed `project_create`、`candidate_prepare`、`candidate_confirm`、`candidate_reject`、`restore_prepare`、`restore_confirm`、`export_prepare`、`export_confirm` 和 `job_cancel` 事务核心；MCP005 新增 `reference_import`/`reference_get`，MCP007/008/009 新增 geometry/appearance/change/quality/version/export。MCP010B/C/D/E/F 源码还提供默认只读的 `operator_catalog_get`、`geometry_program_hash`、`material_pack_get`、`render_pass_get`、`silhouette_target_get`、`camera_fit_prepare`、`silhouette_fit_prepare`、`part_contour_fit_prepare`、`silhouette_part_error_get`、`silhouette_candidate_compare`、`boundary_error_get`，以及 C 的 `reference_compare_prepare`、`visual_review_submit`、`human_visual_review_submit` 显式证据工具；F 的 `reference_mask_prepare`/`reference_mask_refine_prepare` 是显式 opt-in 的 CAS target 写入。Agentic 另提供 `scene_observe_get`、`design_stage_plan_get`、`critic_report_get`、`visual_evidence_bundle_get`、`session_get`、`checkpoint_get`、`design_action_run_get` read surface，以及 approval-gated `session_create_or_resume`、`checkpoint_prepare`、`checkpoint_restore_prepare`、`design_action_run_prepare`。后者当前只执行单 Part `primary-form` bounded action，并锁定 confirm/export。因此当前 source manifest 是 41 read + 33 write = 74，合同总数为 129。除 `capabilities_get`、`runtime_status`、`doctor` 外，MCP session 必须先读 `ponytail-preflight@0.1.0`；该会话 policy 不新增模型或 Provider，也不改变 Runtime 唯一写者模型。固定 renderer、D/E AssetPack 与真实机器人质量边界保持原有描述；轮廓 target/camera/Rig/SDF/Part/candidate compare 的调用顺序和停止规则见 `docs/CODEX_SILHOUETTE_FIT_WORKFLOW.md` 与 `docs/CODEX_CONTOUR_SKILL_PACK.md`。这些 source Gate、Agentic projection、durable prepare/readback 和 Primary Form action-run Gate 仍不等于用户图片 likeness、PBR、人评、packaged/live、Repair execution 或 360 PASS。

2026-08-08 宿主验收记录：Computer Use 对 `com.openai.codex` 的只读状态请求被主机安全边界拒绝；Codex in-app Browser 连接成功但没有当前或用户标签页。该自动化 surface 仍单独记录为 BLOCKED，但不覆盖用户提供的 Desktop 握手/只读证据，也不把 IDE 变成 P0 要求。

### 2.1 MCP003 宿主验收 Runbook

这一步必须在用户拥有的 Codex 宿主中执行；仓库脚本和官方 SDK 只能证明协议互操作，不能替代宿主连接。

前置准备：

1. 使用当前分支构建 MCP 二进制到临时目录，不覆盖用户安装：
   `CARGO_TARGET_DIR=/tmp/forgecad-mcp003-cargo-target script/with_rust_toolchain.sh cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml -p forgecad-mcp --offline`
2. 先运行 `FORGECAD_MCP_COMMAND=/tmp/forgecad-mcp003-cargo-target/debug/forgecad-mcp npm run mcp003:stdio`；失败时先修复 MCP003，不得进入宿主判定。
3. 使用 `config/codex/desktop.toml`、`cli.toml` 或 `ide.toml` 的字段。MVP 安装路径直接使用 `forgecad-mcp`；它先完成 stdio initialize，再异步启动同版本 Runtime，并通过受保护 handoff 连接 authenticated IPC。原始 protocol probe 也直接使用 `forgecad-mcp` 和 Runtime socket/token 变量名。配置不复制 token、API key、绝对路径或真实用户附件。

认证 CLI 的可重复探测（仅在用户明确执行时运行）：

```bash
FORGECAD_MCP_COMMAND=/tmp/forgecad-mcp003-cargo-target/debug/forgecad-mcp \
  python3 scripts/probe_mcp003_codex_cli.py --execute --mode read-only
FORGECAD_MCP_COMMAND=/tmp/forgecad-mcp003-cargo-target/debug/forgecad-mcp \
  python3 scripts/probe_mcp003_codex_cli.py --execute --mode version-mismatch
```

脚本使用隔离临时目录、`--ephemeral`、`--ignore-user-config` 和只读沙箱；默认不启动 Codex、不联网、不写入。只读模式要求两个工具各成功一次（完成顺序不作为合同），版本模式要求无工具调用且宿主报告 fail-closed；它是验收辅助，不替代 required Desktop/CLI 宿主证据，也不把 IDE 或官方 conformance 变成当前 P0 Gate。

每个 REQUIRED 宿主都必须按以下只读序列执行：

1. 发现 `forgecad` Server，并记录 Server 名称和版本；
2. `initialize` 使用宿主实际发送的 `protocolVersion`：当前 Codex stdio 默认观测为 `2025-06-18`，官方 2025-era 客户端可使用 `2025-11-25`；两者都必须在响应中原样协商；空 `capabilities` 和非空 `clientInfo` 仍是必需字段；
3. `capabilities_get`、`project_list`、`resources/list`；
4. `resources/read` 读取 `forgecad://capabilities`；
5. `tools/call` 调用 `selection_get` 和 `version_list`，确认没有创建项目、Job 或版本；
6. 记录宿主 transcript 或截图、工具结果、Runtime 日志中的 request/response hash；不得记录 token、附件绝对路径或模型私有内容。

预期结果：新 source-built MCP 的默认工具数量为 21 且全部 `readOnlyHint=true`；至少发现 `forgecad://capabilities`；capabilities MIME 为 `application/json`；`selection_get.available=false`；没有 ForgeCAD 项目、Job、模型或版本写入。MCP003 历史 read-only receipt 中的 17 个工具与 MCP010A installed Dev.app 的 30-tool receipt 都必须原样保留，不可反向当作 current source count。任何异常都记为 `connection=BLOCKED` 或 `read_only_e2e=BLOCKED`，不能降级为 PASS。

宿主专用动作：

- Desktop：在 Codex MCP 设置中加载 ForgeCAD Server，重启/重新打开新线程后执行上述序列；不要用本仓库自动化读取 Codex Desktop UI。
- CLI：用临时 `-c` 覆盖或用户明确配置加载 `config/codex/cli.toml`，在新的 Codex CLI 会话中执行上述序列；`codex mcp get` 只证明配置发现，不证明连接。
- IDE：保留 `config/codex/ide.toml` 作为未来兼容基线；本轮不安装 VS Code、不启动 Codex IDE，也不执行 IDE 连接或 read-only E2E。未来建设 Skill SDK、插件开发生态或第三方开发者模式时，再单独把 IDE 升级为 REQUIRED。

Desktop 的 forced mismatch 不得通过代理或请求重写伪造：`launchctl setenv CODEX_MCP_PROTOCOL_VERSION=2026-07-28` 后真实 Desktop 仍发送 `2025-06-18`，因此只记录 `HOST_OVERRIDE_IGNORED / NOT_APPLICABLE`。协议负面测试由 ForgeCAD stdio/raw probe 和真实 Codex CLI `2026-07-28` 回合共同承担：必须明确 `CONTRACT_VERSION_UNSUPPORTED`/unsupported-protocol、无工具调用、无静默降级、无副作用；宿主不能为了通过测试而伪造客户端名称，`clientInfo.name` 不是认证。

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

`FGC-MCP003` required protocol adapter、Codex Desktop 和 Codex CLI 均有真实 PASS。`FGC-MCP004` 已按单用户事务基座范围标为 done；`FGC-MCP005` 已按 PNG/JPEG reference admission、CAS readback 和真实 Codex CLI image-attachment 范围标为 done，Desktop bridge 仍为 `NOT_RUN / unavailable`；`FGC-MCP006` 已按 typed contracts、十个独立 declarative Bundle、Recipe/validator/fixture/license/SBOM/provenance Gate 范围标为 done；`FGC-MCP007` 已按有界多 Part geometry、确定性 GLB 和严格 readback 范围标为 done；`FGC-MCP008` 已按 bounded UV/tangent/PBR/fixed render/Viewer focused 范围标为 done；`FGC-MCP009` 已按 limited quality/stable-Part change/immutable version/restore/CAS export 和真实 Codex CLI 十二调用 host 范围标为 done；`FGC-MCP010A` 已按真实 Desktop 30-tool/Ready/cohort/project readback 激活范围标为 done。pixel similarity、human score、完整 Desktop 3D write 和 signed packaged Desktop write 仍是 `BLOCKED/NOT_RUN`；IDE、其他 MCP Client 和当前 transport 不匹配的官方 conformance 继续为未来/非阻塞状态。

### 2.2 MCP004 Runtime/IPC 事务 Runbook

当前只验证无图片、无几何执行的 typed transaction：Codex 先调用 `project_create`，再以 `request.typed=diagnostic` 调用 `candidate_prepare`；Runtime 自己生成 CAS 合同对象和 contract-only quality report，Codex/MCP 不能伪造 quality pass；最后通过 authenticated IPC 调用 `candidate_confirm` 或 `candidate_reject`。Restore 先从 project 内 confirmed 历史 version 调用 `restore_prepare`，再批准创建当前 head 的新子版本；diagnostic export 调用 `export_prepare` 生成 path-free manifest，再由 `export_confirm` 绑定审批和 idempotency。每次 confirm 必须检查 project/base/candidate/prepared object/hash/quality/approval/expiry/idempotency，并核对版本、snapshot、candidate、audit 和重启 readback 的 hash/lineage。

已通过 focused negative cases：hash mismatch、stale base、quality hard fail、approval expiry、不同 request 复用同一 idempotency key、重复 confirm、reject、restore source/stale mismatch、diagnostic export mismatch 和 cancelled Job；失败路径不得写 immutable version、移动历史 head 或生成已确认 export。MCP004 adapter 还通过默认只读、写工具确认标记、disabled typed error 和 authenticated IPC opt-in prepare 测试。Codex CLI diagnostic write E2E 与 MCP010A Desktop 最小 activation write probe 已 PASS；完整 Codex Desktop 3D write、reference attachment、Geometry/Render/Quality、生产文件/GLB export 和 packaged Viewer 仍不能填写为 PASS。

### 2.3 MCP004 内置 Runtime supervisor、Tauri resource bundle 与 Codex CLI diagnostic write E2E

仓库现在提供两个受控入口。`forgecad-runtime serve` 用于独立诊断；正常配置直接使用 `forgecad-mcp`。MCP 先启动不依赖 Runtime 的 stdio，再通过受保护的 `ready.json`/status handoff 动态连接 Runtime；Runtime 进入 Starting/Ready/Degraded/Restarting 时，MCP stdio 不退出，依赖调用返回结构化 `RUNTIME_UNAVAILABLE`。MCP 不打开 SQLite/CAS，也不成为第二个 Runtime writer；最多一次有界重启带 100ms backoff。诊断 fixture、confirm、签名、deep codesign、spctl 和 notarization 只在独立测试/发布 Gate 中运行，正常 `serve --stdio` 不读取 fixture 环境。

```bash
CARGO_TARGET_DIR=/tmp/forgecad-mcp004-launcher-target \
  script/with_rust_toolchain.sh cargo build \
  --manifest-path apps/desktop/src-tauri/Cargo.toml \
  -p forgecad-runtime --bin forgecad-runtime -p forgecad-mcp --offline

script/test_mcp004.sh

python3 scripts/probe_mcp004_codex_cli.py --execute \
  --runtime-command /tmp/forgecad-mcp004-launcher-target/debug/forgecad-runtime \
  --mcp-command /tmp/forgecad-mcp004-launcher-target/debug/forgecad-mcp
```

2026-08-08 的真实 Codex CLI 结果为 PASS：`project_create → candidate_prepare → candidate_confirm → restore_prepare → restore_confirm → export_prepare → export_confirm` 全部 completed，随后同一临时 Runtime 的 Tauri Viewer read model 读回 1 个项目、2 个版本，MCP 也读回相同状态；无无关副作用、无用户数据修改、无图片上传、无 3D 生成、无生产文件导出。原始调用参数不进入证据，脱敏 receipt 为 `docs/evidence/mcp004/codex-cli-write-e2e.json`；开发 launcher/IPC 过程证据为 `docs/evidence/mcp004/launcher-ipc.json`。

内置 supervisor 的新本地回归结果为 PASS：Runtime 缺失时 initialize 仍成功；Runtime ready 后 child crash 时 stdio 仍响应，状态经过一次 bounded restart 后为 `Degraded`，依赖调用返回 `RUNTIME_UNAVAILABLE`；当前 source 的 28 个只读工具和 18 个显式 write tools 的 approval metadata 保持，并包含轮廓目标、37 个覆盖全局尺度的粗候选加局部探针相机拟合和边界误差读取。旧 17-read Host probe 仅作为历史记录保留，不作为本次 source MCP010B–F 运行结果。

旧 standalone Host bundle 记录仅作为 `SUPERSEDED` 历史证据保留；MCP010A Dev.app 的 30-tool Desktop restart/live Gate 已通过并保留为历史。MCP010B f488 Dev.app 的 ad-hoc deep-strict、三资源 package verify、隔离 Ready/project_create、fixed-sibling packaged Worker V2 semantic-Part graph raw probe 与真实 Codex CLI structural probe也均为历史 cohort evidence。历史 `bfa56ac…de9` Dev.app 的 package/isolated/raw/real-Codex/live receipts均保留；当前 `d9c23b…ac0bd` Dev.app 已通过 ad-hoc/package、isolated Ready/project、raw V2、matching Worker、real-Codex structural probes和用户完整重启后的 live Desktop structural activation；CLI candidate未确认，仅为 12 Parts/896 triangles/161104-byte GLB。live 只证明 32 工具、Ready/cohort/catalog/hash/project readback，不证明视觉质量或 3D write。以上开发证据都不是 Developer ID/notarization 或完整 production packaged Desktop E2E；后者继续是 `BLOCKED`/`NOT_RUN`，详细历史签名证据见 `docs/evidence/mcp004/macos-signing-diagnostic.json`。禁止通过代理或请求重写制造 Desktop mismatch 或 write PASS。

## 3. Codex instructions

随 Server 提供的 instructions 必须要求 Codex：

- 在任何设计 tool 或其他 Skill 前先读 `ponytail-preflight@0.1.0`；`capabilities_get`、`runtime_status`、`doctor` 仅可作 bootstrap diagnostics；
- 随后读取 `capabilities_get`、`project_list` 和需要的 snapshot/resource；
- 不猜测不可用能力；
- 对含糊对象、缺失视图、尺寸或材质先向用户澄清；
- 只提交公开 typed Schema，不提交任意脚本；
- 长任务通过 job 轮询并允许用户取消；
- 读取结构、渲染和质量证据后再建议确认；
- 将硬门失败直接告诉用户，不用语言评价覆盖；
- 任何永久写入、恢复、安装 Skill 和导出都使用 Codex write approval；
- 不把 Viewer 截图、自然语言或单一 GLB 可打开等同高质量通过。

## 4. Viewer 联动

Viewer 的相机、选择、隔离和临时爆炸距离是 ephemeral UI state。MCP003 的 `selection_get` 和 selection resource 会诚实返回 `available=false`；MCP009 的 `change_prepare` 可由 Codex 直接提交稳定 Part ID，但当前 Viewer selection UI 尚未接入，不把空选择投影成稳定 Part ID。Viewer 不直接发送 prompt，也不拥有会话。

候选完成时 Runtime 发布本地事件，Viewer 刷新 read model；Codex 无需保证 Viewer 打开。Viewer 关闭时所有 compile/render/evaluate、版本和导出语义保持不变。

## 5. 安装与更新

安装器交付同版本、同签名的 Runtime、MCP、Viewer 和 workers，并为用户生成 Codex MCP 配置。更新流程先验证签名、合同兼容和数据库备份，再原子切换；失败回滚整套二进制，不能混用不同合同版本。当前仅完成 unsigned resource placement，签名链路未通过。

不在配置中保存 OpenAI API Key。Codex 的身份、订阅和模型调用由 Codex 自身管理，ForgeCAD 不读取或复制这些凭据。

## 6. 真实验收脚本

MVP Gate 先在开发构建和真实 Codex CLI 完成步骤 2–9 的首个硬表面 vertical slice；最终 packaged Gate 在 MCP013 用普通用户安装路径重跑：

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
