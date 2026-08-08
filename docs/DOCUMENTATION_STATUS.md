# ForgeCAD 当前状态账本

版本：2026-08-08 · 分支：`main` · 任务：`FGC-MCP003 in_progress`

## 当前事实

旧桌面工作台、内置模型/Provider、App Server/Protocol、Python Agent、旧脚本、旧合同和旧迁移目录已经从工作树删除；新 Viewer、Runtime、MCP、Worker 协议、Runtime V1 migration、完整首批 contracts、CAS、SQLite repository、writer lease 和 authenticated local IPC 已写入。`WushenForgeLibrary` 未修改，reset 归档位于 `/tmp/forgecad-mcp001-20260807`。

当前代码只提供诊断级 Runtime：MCP 已实现 `2025-11-25` canonical 与 Codex `2025-06-18` compatibility 初始化协商、只读 tools/resources、URI 白名单、版本不兼容 fail closed 和 Codex 三宿主配置基线；Runtime 已具备 V1 Schema、SQLite/CAS、事务回滚、备份恢复、单写者租约和本地认证 IPC。几何、渲染、参考导入、Skill 执行和永久写入仍 fail closed。Desktop 只显示 Runtime Viewer，不提供聊天、上传、模型选择或 API Key。

## 能力账本

| 能力 | 状态 | 证据/缺口 |
|---|---|---|
| Codex-only 产品边界 | 已实现 | ADR-0025、Viewer 文案、无模型依赖；真实三宿主兼容待 MCP003 |
| 旧产品硬切 | 已实现 | 删除搜索、恢复包、文档/CI Gate 和新骨架 evidence 已通过；后续任务不得恢复旧入口 |
| 可恢复 reset | 已实现 | 分支、tracked diff、untracked/Library archive、恢复读取验证；`docs/evidence/mcp001/manifest.json` |
| Runtime contracts | 已实现（MCP003 范围） | 15 个 JSON Schema、Rust records、canonical hash、resource/selection records 和 manifest 无漂移检查通过；跨宿主 conformance 未运行 |
| SQLite Runtime V1 | 已实现（MCP002 范围） | 完整 Runtime V1 migration、Project/Candidate/Version/Snapshot/Job/Event/Audit/Object 表、事务回滚、重启、旧库拒绝通过；长期 Job/confirm 事务待 MCP004/011 |
| CAS | 已实现（MCP002 范围） | SHA-256、临时文件+fsync+原子 rename、容量/缺失/篡改/metadata mismatch、备份恢复通过；GC/reachability policy 待 MCP011 |
| Runtime 单写者 | 已实现（MCP002 范围） | SQLite lease、heartbeat、TTL recovery、并发双 writer Gate 通过；packaged kill-9/升级恢复待 MCP011/013 |
| authenticated local IPC | 已实现（MCP002 范围） | Unix socket 0600、每次启动生成且仅进程内持有的 token、常量时间比较、错误 token fail closed、Runtime dispatch smoke 通过；三宿主 packaged wiring 待 MCP013 |
| MCP stdio/resources | 部分实现 | MCP `2025-11-25` canonical + Codex `2025-06-18` compatibility initialize、14 个只读 tools、resources/list/read/templates、annotations、URI/contract fail closed、原始 stdio probe、官方 SDK stdio probe、认证 Codex CLI 和用户提供的 Desktop 只读工具/资源回合已通过；Desktop initialize/version-mismatch 证据、IDE E2E 与官方 conformance 仍未完成 |
| Runtime Viewer | 已实现（诊断级） | `apps/desktop/src/features/runtime-viewer` typecheck/build 通过；无模型展示能力 |
| Geometry/Render workers | 部分实现 | stdin/stdout typed unavailable skeleton；受限编译器待 MCP007/009 |
| Skill Bundle | 目标设计 | 标准已写；签名、SBOM、Registry、Benchmark 待 MCP006 |
| 几何/轮廓/比例/局部修改 | 目标设计 | 新合同和 Worker 待 MCP007 |
| UV/PBR/纹理/材质 | 目标设计 | Appearance Compiler 待 MCP008 |
| 参考比较/视觉评审 | 目标设计 | Render/Quality Compiler 和真人门待 MCP009/013 |
| 版本/回退/爆炸图/导出 | 目标设计 | Runtime 事务待 MCP004/010 |

## 明确未运行/阻断

- Rust workspace、core/store/runtime IPC tests、worker checks、MCP003 protocol/host baseline、Viewer typecheck/build 和独立临时 Cargo target 检查已有 PASS；本轮 `npm run release:mcp003` 的默认 Tauri 增量检查因旧 target 长时间无输出被安全中断（exit 130），所以不能把聚合 release Gate 写成 PASS；
- `docs/evidence/mcp003/host-matrix.json` 中本地 protocol adapter、原始 stdio probe、官方 SDK stdio probe、Codex CLI 配置发现/只读回合和现代协议不兼容 smoke 为 PASS；用户提供的 Desktop 截图/transcript 证明只读工具与资源回合 PASS，但 initialize.protocolVersion 未记录且 host version mismatch 未运行；Computer Use 自动化仍被安全边界拒绝，IDE discovery/connection 仍为 BLOCKED；
- 尚未证明 Codex Desktop/CLI/IDE 能把图片字节传给本地 MCP；
- 尚未进行 packaged Runtime/Viewer、真实 Codex 附件、完整 kill-9/磁盘配额注入、视觉相似度和真人评分；
- 不得将旧 U004/C/E/K/F 测试、旧 GLB 或旧截图当作新产品证据。
