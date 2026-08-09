# ForgeCAD 当前交接

更新时间：2026-08-09
分支：`codex/mcp010a-legacy-cleanup`；工作树 dirty，必须保留
任务状态：`FGC-MCP005–FGC-MCP009 done（MVP host golden path）`；`FGC-MCP010A in_progress`，`MCP010B–F blocked`

## 1. 本次范围决策

产品按个人单用户 MVP 收口，不再让生产签名、后台服务治理或第三方插件市场阻塞真实 3D：

- 运行时只有 `forgecad-mcp`、`forgecad-runtime`、按 Job 启动的 typed workers 和可选 read-only Viewer；
- Runtime 用 OS 文件锁单写者，无 TTL/heartbeat；MCP initialize 不等 Runtime，Runtime 异常最多一次简单重启；
- MCP004 作为 candidate/approval/version/restore 的事务基座关闭；
- MVP 主线 MCP005 reference → MCP006 typed contracts/Skills → MCP007 geometry → MCP008 PBR/render/Viewer → MCP009 quality/change/version/export 已完成 focused Gate；真实 Codex CLI 的授权图片到 CAS GLB 十二调用 host golden path 也已 PASS；当前只剩真人/像素相似度、Desktop write 和正式分发 Gate，不再扩展基础设施；
- 当前工具、Skill 和 GitHub 候选的可执行边界统一见 `docs/MVP_TOOL_CATALOG.md`；没有 `accepted` 第三方依赖，不得因为 GitHub 链接而修改 lockfile；
- MCP010A–F 负责首个硬表面参考质量产品化；复杂 Job recovery/GC 属 MCP011，通用第三方 Skill/AssetPack 生命周期属 MCP012，signing/notarization、packaged Desktop、升级回滚和跨类别真人门属 MCP013。

范围调整没有把未完成能力改成 PASS；`docs/evidence/mcp004/manifest.json` 保留历史 `in_progress`、BLOCKED 和 NOT_RUN 现场。

## 2. 当前已实现

- MCP001：旧 Provider/Agent/App Server/工作台硬切，新 Rust/Tauri/Worker/contracts 骨架；旧用户 Library 未修改；
- MCP002：Project/Candidate/Version/Snapshot/Job/Audit/CAS contracts、SQLite V1、atomic CAS、backup/restore、OS writer lock、authenticated Unix IPC；
- MCP003：MCP 2025-era stdio/resources/read-only tools，Codex `2025-06-18` compatibility，Desktop/CLI required read-only evidence，CLI modern mismatch fail closed；
- MCP004：candidate prepare、Job/audit、quality/approval/idempotent confirm/reject、restore-as-new-version、path-free diagnostic export、authenticated IPC、显式 opt-in MCP writes；
- `forgecad-mcp` 拥有 stdio并异步启动/连接同一数据根的共享 Runtime；短时 launcher flock 只做启动选主，Runtime 持有的 `runtime.writer.lock` 才是最终唯一写者。正常适配器退出不终止已经 Ready 的 Runtime，显式 shutdown/update 才停止；缺失/失败/ready 后 crash 时 stdio 存活，依赖调用返回 `RUNTIME_UNAVAILABLE`，最多一次 restart。最终源码的 MCP 26/26 与 shared lifecycle Gate 已 PASS；同 cohort Dev.app 也已重建并隔离验证，当前只等待第二次 Desktop 完整重启；
- 真实 Codex CLI 完成 diagnostic project/create/confirm/restore/export；Viewer 通过 authenticated IPC 读回同一项目/版本；
- MCP005 已完成 PNG/JPEG admission、ReferenceEvidence/CAS readback、authorized-root/symlink/path/hash/MIME negative tests 和真实 Codex CLI image-attachment E2E；证据位于 `docs/evidence/mcp005/`；原图路径/字节未进入仓库或 receipt；
- MCP006 已完成：44 个合同、十项 first-party registry、十个独立 `bundles/<skill_id>/0.1.0`、`skill_list/get`、Skill resource、trust hash、安全 allowlist、Recipe canonical hash、DAG/单位/finite/预算 validator、合成正/负 fixture、每 Bundle benchmark receipt、LICENSE/NOTICE、SPDX SBOM 和 provenance 已落地；`uv-pbr` 已标记为 product-owned bounded geometry consumer；`scripts/materialize_mcp006_bundles.py`、`scripts/check_mcp006_skills.py` 与 Runtime/MCP focused tests 已通过；正式 distribution signature 仍延后到 MCP012–013；
- 真实 Codex CLI 已完成只读 `capabilities_get → skill_list → skill_get`，证据见 `docs/evidence/mcp006/codex-cli-skill-registry-e2e.json`；它只证明 registry metadata 传输，不证明几何/渲染/质量；
- MCP007 已完成：product-owned bounded GeometryProgram compiler、box/cylinder/sphere、14 个语义机器人 Part fixture、finite/index/budget/lineage 检查、确定性 GLB、`ArtifactReadback@1`、authenticated MCP geometry/readback、Viewer candidate/artifact read model。`npm run mcp007:test` PASS；真实 Codex CLI 已用用户授权 PNG 完成 `project_create → reference_import → geometry_prepare → artifact_readback_get`，14 parts/516 triangles/validator passed，证据见 `docs/evidence/mcp007/`；它只证明 typed geometry host slice，不单独证明外观或视觉相似度；
- MCP008 已完成：`AppearanceProgram@1` hash-bound material zones、UV/tangent、glTF PBR、四个固定 PNG pass、Runtime readback、Three.js GLB canvas 和 `npm run mcp008:test`；证据见 `docs/evidence/mcp008/`；未实现纹理烘焙/UDIM/全 AOV；
- MCP009 已完成 MVP host golden path：`quality_get`（limited aspect compare）、`version_diff`、`change_prepare`、immutable confirm/reject/restore、`mvp-glb` CAS export receipt；`npm run mcp009:test` 的 24 Runtime tests + 16 MCP tests PASS；真实 Codex CLI 已完成十二调用 reference→geometry→appearance→quality→confirm→version→CAS GLB export，证据见 `docs/evidence/mcp009/`；
- `npm run release:mcp004` 历史 aggregate PASS；最终修复源码的 `script/test_mcp004.sh` 已 PASS（MCP 26/26 + shared lifecycle），`npm run release:mvp` 已 exit 0（Runtime 30/30、MCP 26/26、44 contracts、MCP005–009、Viewer/Tauri、docs/security）。cohort `7a8fddf99c57893db93fe1bdd98ab65302bd890d191026495cbbc63ae4652064` 的 Dev.app 已安装并通过 ad-hoc deep-strict、`package:verify` 与隔离探针。真人/像素相似度/production packaged gate 仍单独记录。
- MCP010A 可恢复旧代码清理已 PASS：恢复包 `20260809-mcp010a-legacy-cleanup` 的 Git bundle、tracked/local archive、worktree patch 和 SHA-256 清单均已验证；旧 Provider/Planner/CSG evaluation、packaged Agent sidecar、Gate07 配置、5 个孤儿 Python Agent、旧虚拟环境/Host/Proxy/生产包/日志/缓存/stale endpoint 已退出当前树或进入私有隔离区；无用 Rust API/依赖和旧 Tauri resource protocol 已移除。两份 standalone Host receipt 只保留为 `docs/evidence/archive/` 下的 `SUPERSEDED` 历史；`output/`、`WushenForgeLibrary`、Runtime V1 和 Codex 历史未修改。

## 3. 当前没有实现或不能宣称

- 像素级 silhouette IoU、landmark/region compare、Codex typed visual review 和真人评分；`quality_get` 只能返回明确 limited 的 aspect-ratio evidence；
- 通用 mesh-delta、只重编受影响 DAG 的优化；`change_prepare` 是稳定 Part + 新 typed program 的有界重编入口；
- 任意本机路径/生产安装包导出；当前 `mvp-glb` 是 CAS-backed hash receipt，文件系统/签名导出留 MCP013；
- 同 hash Viewer/Runtime/restart 的真实 Codex host visual E2E；
- Developer ID/notarization/clean install/upgrade rollback；
- 通用跨类别高质量与独立真人门。

因此当前软件已经可以在开发构建中运行“用户授权图片 → typed geometry/appearance → GLB/render → candidate → confirm → CAS-only MVP GLB export”的真实 Codex MVP 闭环；仍不能把 aspect-ratio limited evidence 写成像素级相似度、真人高质量验收或通用图生 3D。

## 4. 当前证据摘要

| Gate | 状态 | 边界 |
|---|---|---|
| contracts/store/runtime/IPC focused | PASS | MCP002/MCP004 基座 |
| MCP protocol + required Desktop/CLI read-only | PASS | 不含附件或视觉写入 |
| MCP004 lifecycle regression | PASS（最终修复源码） | MCP 26/26 与 shared lifecycle Gate PASS；覆盖 stdio survival、一次 restart、RUNTIME_BUSY、无只读副作用、共享 Runtime idle owner/passive takeover 与 rogue pre-auth client 隔离 |
| Codex CLI diagnostic write | PASS | 非视觉 contract-only candidate |
| Viewer authenticated read model + GLB canvas | PASS focused | 项目/候选/GLB bytes/版本投影；Three.js scene 只读、无 Runtime 写入 |
| `npm run release:mcp004` | PASS | 当前基座 aggregate；不等于 MVP |
| Reference attachment | PASS（CLI）/ NOT_RUN（Desktop bridge） | MCP005 evidence；Desktop 不能证明 attachment bytes |
| Geometry | PASS focused + real Codex CLI | MCP007 worker/Runtime/MCP/Viewer focused PASS；real CLI 14-part/516-triangle receipt PASS；Desktop write仍未运行 |
| Appearance/Render | PASS focused + real Codex CLI | MCP008 bounded UV/tangent/PBR + four fixed passes；MCP009 receipt含真实 appearance/readback |
| Quality/Change/Version/Export | PASS MVP host golden path | MCP009 limited quality + approval/version + CAS-backed mvp-glb；pixel similarity/human gate NOT_RUN |
| MCP010A authority/dev activation | IN_PROGRESS / DESKTOP ATTEMPT 1 FAIL / ATTEMPT 2 NOT_RUN | 第一次完整重启只见 17 个只读工具，`project_create` 不可见，Runtime 实际不可用且未创建项目；失败 receipt 保留。共享 Runtime/IPC 修复、current aggregate、cohort `7a8fddf99c57893db93fe1bdd98ab65302bd890d191026495cbbc63ae4652064` 重建安装、包验证与隔离探针均 PASS；第二次完整重启待用户执行 |
| signed/notarized packaged Desktop | BLOCKED / NOT_RUN | MCP013；历史 codesign 为 `errSecInternalComponent` |
| IDE/其他 MCP Client/official transport conformance | OPTIONAL_NOT_IN_SCOPE | 不阻塞个人 MVP |

## 5. 当前任务：FGC-MCP010A（in_progress）

用户已批准 `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`。本轮只能完成 010A，不要并行修改 010B–F 的合同/Worker/Viewer，也不要重新实现 MCP008/009：

1. 第一次用户重启后的 live Gate 已运行并 `FAIL`；保留 `docs/evidence/mcp010a/codex-desktop-post-restart-failed.json`，不得改写成 `NOT_RUN`、`BLOCKED` 或 PASS；
2. 首次失败观察为：宿主加载了开发 MCP，但只列出 17 个只读工具；`project_create` 不可见；`capabilities_get`/`project_list` 返回 `RUNTIME_UNAVAILABLE`；没有活 Runtime，也没有项目/版本/模型写入；
3. 用户 Codex 配置已备份、继续指向 Dev.app 内的 `forgecad-mcp`，并改为显式 server environment write opt-in；仓库配置仍保持通用命令，无 token、fixture data dir 或用户绝对路径；
4. 共享 Runtime 修复已完成：多个 MCP 适配器通过 launcher flock 做短时启动选主，`runtime.writer.lock` 保持最终唯一写者；已经 Ready 的 Runtime 不随某个适配器退出而停止；stale handoff 和未认证/坏客户端不能阻塞恢复。最终源码 `script/test_mcp004.sh`、MCP 26/26、Runtime 30/30 与 `release:mvp` 均 PASS；
5. 同一源码 revision 的 MCP、Runtime、Geometry Worker 和 Viewer 已重建并安装。当前 cohort 为 `7a8fddf99c57893db93fe1bdd98ab65302bd890d191026495cbbc63ae4652064`，ad-hoc deep-strict、`package:verify` 与隔离探针 PASS；探针协商 `2025-06-18`、观察到 `Ready` 和 cohort match、完成隔离 `project_create`，未触碰持久用户数据。先前 cohort `e5fd7da79576fd022894838c5ab9b0532b7aef735abc42b86f3283e43532ea91` 只描述 attempt 1 的旧开发包；Worker 虽会打包，Runtime 在 010D 前仍不宣称独立进程已激活；
6. 现在请用户第二次完整重启 Codex Desktop；只有 live `capabilities_get`、`project_list`、临时 `project_create`、Runtime `Ready`、30 个工具和 MCP/Runtime 相同 build cohort 均成功，010A 才能 done；
7. 第二次成功 receipt 形成前，010A 保持 `in_progress`，010B–F 保持 `blocked`。当前仍是 44 Schema、17 read + 13 write tools 和 Skill `0.1.0`。

本次文档校正已运行 `release:docs-walkthrough`、`repository:integrity`、`release:safety-scope`、`release:secrets-files`、`release:license-sbom` 和 `git diff --check`，均 PASS；这些只证明文档/仓库边界一致，不替代 Desktop attempt 2。

详细的 A–F owned paths、目标合同、质量阈值和 011–013 分界见 `docs/MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`。当前单张参考最多支持 `PARTIAL_VISIBLE_VIEW_PASS`；补齐 front/back/left/right/rear-three-quarter 全身参考前，`HQ_360_PASS=BLOCKED_REFERENCE_COVERAGE`。

MCP007–009 仍禁止顺带接入 BlenderMCP、Python CAD、远程 image-to-3D、资产下载或生产签名；外部几何库只能按 adoption receipt 进入隔离 benchmark。

## 6. 工作树保护

当前仓库已有大量未提交修改和未跟踪 MCP004 文件，均视为用户工作。不得 `git reset --hard`、`git clean`、checkout 覆盖或删除 `WushenForgeLibrary`；除非用户明确要求，不 commit/push/merge。

每轮基线：

```bash
git status -sb
git diff --check
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
```

完成定义、外部项目流程和 Goal 状态句分别见 `CODEX_DEFINITION_OF_DONE.md`、`EXTERNAL_PROJECT_ADOPTION.md` 和 `LUNA_GOAL_EXECUTION_GUIDE.md`。
