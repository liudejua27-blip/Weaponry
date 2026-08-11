# ForgeCAD 当前交接

更新时间：2026-08-11
分支：`main`；工作树包含受控 MCP010B/C 修改
任务状态：`FGC-MCP005–FGC-MCP010A done（MVP host golden path + Dev activation）`；`MCP010B blocked/deferred（Darwin OS hard cap NOT_RUN）`；`MCP010C in_progress/source-focused PASS_WITH_UNRUN_VISUAL_GATES`；`MCP010D–F blocked`

> MCP010B/C reconciliation：MCP006 的 44-contract 与已保存的 50/52-contract、3c/f488/d9 receipts是历史或结构事实。当前源码共 59 contracts、20 read + 16 opt-in write = 36 tools；MCP010C 的固定 renderer、九 AOV、candidate-bound comparison、MCP image block、Codex/human review 已通过 source raw Gate，真实 Codex CLI 也已完成六 turn/32 MCP 调用的同 cohort C transport receipt。真实机器人 PNG likeness threshold 仍为 `FAIL_QUALITY_TARGET_NOT_MET`（silhouette IoU 0.5132、boundary F1 0.1441）；Viewer compare source implementation 已通过本地 IPC/构建测试，但 B Darwin OS 总内存硬门、packaged/current-cohort Viewer/C、人评阈值、PBR/纹理、export/restart hash/360仍未完成，不得把 C structural/raw/CLI transport PASS 写成高质量视觉 PASS。

> 最新开发包 `d9c23b…ac0bd` 在 primitive-blockout 知识分支修正后重新构建并安装：ad-hoc/package、隔离 Ready/project、raw V2 和真实 Codex CLI V2 structural 均 PASS，`primitive-blockout@0.2.0` 为 active，候选仍未确认（12 Parts/896 triangles/161104-byte GLB）。用户完整重启后 d9 已成为当前 live Desktop cohort，已重新验证 32 工具、Ready、cohort/catalog/hash、active Skill 和临时项目 readback；该结构证据不宣称视觉/PBR/人评/360。

> 2026-08-10 真实用户授权图片演练已另外记录：`project-822d5513ad41499290910913cbc2bfd1` / `reference-35bc12fe88f349c9ba3590b271fb9130` / `candidate-205dce23763d4fdb98f7860699cd78b4`。V2 strict geometry/readback 通过（23 Parts、9964 triangles、1,592,884-byte GLB、完整 coverage/零 integrity errors），但 limited aspect quality 为 `0.5466 < 0.55`，candidate 未确认；这不是视觉相似度或 PBR PASS。完整脱敏 receipt 见 `docs/evidence/mcp010b/real-reference-robot-structural-run.json`。

> Codex 当前单图调用 checklist 见 `docs/CODEX_SINGLE_REFERENCE_OPERATING_GUIDE.md`。它只编排已激活的 catalog/GeometryProgram@2/readback 路线，并明确 limited quality、unknown region、confirm/export 和 C–F 交接停止条件；它不是新的运行时 Skill 或视觉质量证据。

> 同一授权参考的第二次 detail blockout 记录在 `docs/evidence/mcp010b/real-reference-robot-detail-blockout.json`：51 Parts、16,496 triangles、2,658,940-byte GLB 的 strict readback 通过，但 limited aspect proxy 为 `0.4604 < 0.55`。这次负向结果证明“增加 primitive 数量”不能替代固定相机、silhouette 和 region 比较；candidate 未确认、未创建 version/export。

> MCP010C source Gate：当前源码已提供固定 512×512 perspective/z-buffer renderer、九个 PNG AOV、本地 reference mask/metrics、`render_pass_get` image block、Codex typed review 和 HumanVisualReview receipt。`script/test_mcp010c.sh`、raw stdio receipt 与真实 Codex CLI C receipt 均已通过结构/传输门；CLI receipt 完成六个短 turn、32 个 ForgeCAD 调用和九 AOV 读取，但真实机器人 likeness 仍为 silhouette IoU `0.5132`、boundary F1 `0.1441`、`QUALITY_TARGET_NOT_MET`。Viewer compare source implementation 已通过本地 IPC/构建测试；packaged/current-cohort Viewer/C、人评阈值、PBR/纹理、export/restart hash 和 HQ_360 仍 NOT_RUN/BLOCKED，详见 `docs/MCP010C_READINESS_AUDIT.md`。

> Packaged C update：当前 Dev.app 安装/包验证/隔离探针、九 AOV raw renderer 和 packaged Codex CLI compare/review transport 已通过，receipt 位于 `docs/evidence/mcp010c/`；这不等于 packaged Viewer UI、人评或 likeness PASS。机器人质量状态仍为 `QUALITY_TARGET_NOT_MET`，Viewer UI、PBR/纹理、export/restart hash 和 HQ_360 继续 `NOT_RUN/BLOCKED`。

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
- `forgecad-mcp` 拥有 stdio并异步启动/连接同一数据根的共享 Runtime；短时 launcher flock 只做启动选主，Runtime 持有的 `runtime.writer.lock` 才是最终唯一写者。正常适配器退出不终止已经 Ready 的 Runtime，显式 shutdown/update 才停止；缺失/失败/ready 后 crash 时 stdio 存活，依赖调用返回 `RUNTIME_UNAVAILABLE`，最多一次 restart。最终源码的 MCP 26/26 与 shared lifecycle Gate 已 PASS；同 cohort Dev.app 已重建、隔离验证，并通过第二次 Desktop live Gate；
- 真实 Codex CLI 完成 diagnostic project/create/confirm/restore/export；Viewer 通过 authenticated IPC 读回同一项目/版本；
- MCP005 已完成 PNG/JPEG admission、ReferenceEvidence/CAS readback、authorized-root/symlink/path/hash/MIME negative tests 和真实 Codex CLI image-attachment E2E；证据位于 `docs/evidence/mcp005/`；原图路径/字节未进入仓库或 receipt；
- MCP006 已完成历史范围：44 个合同、十项 first-party registry、十个独立 `bundles/<skill_id>/0.1.0`、`skill_list/get`、Skill resource、trust hash、安全 allowlist、Recipe canonical hash、DAG/单位/finite/预算 validator、合成正/负 fixture、每 Bundle benchmark receipt、LICENSE/NOTICE、SPDX SBOM 和 provenance 已落地；MCP010B 当前源码另有 `primitive-blockout@0.2.0`，其 primitive@2 consumer 通过 Runtime Skill truth overlay 被标为 active；`uv-pbr` 已标记为 product-owned bounded geometry consumer；`scripts/materialize_mcp006_bundles.py`、`scripts/check_mcp006_skills.py` 与 Runtime/MCP focused tests 已通过；正式 distribution signature 仍延后到 MCP012–013；
- 真实 Codex CLI 已完成只读 `capabilities_get → skill_list → skill_get`，证据见 `docs/evidence/mcp006/codex-cli-skill-registry-e2e.json`；它只证明 registry metadata 传输，不证明几何/渲染/质量；
- MCP007 已完成：product-owned bounded GeometryProgram compiler、box/cylinder/sphere、14 个语义机器人 Part fixture、finite/index/budget/lineage 检查、确定性 GLB、`ArtifactReadback@1`、authenticated MCP geometry/readback、Viewer candidate/artifact read model。`npm run mcp007:test` PASS；真实 Codex CLI 已用用户授权 PNG 完成 `project_create → reference_import → geometry_prepare → artifact_readback_get`，14 parts/516 triangles/validator passed，证据见 `docs/evidence/mcp007/`；它只证明 typed geometry host slice，不单独证明外观或视觉相似度；
- MCP008 已完成：`AppearanceProgram@1` hash-bound material zones、UV/tangent、glTF PBR、四个兼容 PNG pass、Runtime readback、Three.js GLB canvas 和 `npm run mcp008:test`；MCP010C 在不改写该历史 RenderSet@1 的前提下新增 RenderSet@2 九 AOV/reference comparison/review source path；纹理烘焙/UDIM/PBR V2 仍未实现；
- MCP009 已完成 MVP host golden path：`quality_get`（limited aspect compare）、`version_diff`、`change_prepare`、immutable confirm/reject/restore、`mvp-glb` CAS export receipt；`npm run mcp009:test` 的 24 Runtime tests + 16 MCP tests PASS；真实 Codex CLI 已完成十二调用 reference→geometry→appearance→quality→confirm→version→CAS GLB export，证据见 `docs/evidence/mcp009/`；
- `npm run release:mcp004`、44/50/52-contract、3c/f488/bfa56/d9 Dev.app 以及其 Codex structural receipts均按历史范围保留，不能改写成 current C source。当前源码 59 contracts、`script/test_mcp010c.sh` 固定 renderer/九 AOV/comparison/review/raw Gate均 PASS；真实 Codex CLI C 另有六 turn/32-call receipt，C synthetic/CLI receipt 与首次真实机器人 receipt 均不构成 likeness PASS，后者已明确记录 `QUALITY_TARGET_NOT_MET`；Viewer compare、packaged/live C、人评阈值、PBR V2、export/restart hash 和 360仍保持 NOT_RUN/BLOCKED。
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
| Geometry | PASS focused + real Codex CLI | MCP007 worker/Runtime/MCP/Viewer focused PASS；real CLI 14-part/516-triangle receipt PASS；完整 Desktop 3D write 仍未运行 |
| Appearance/Render | PASS focused + real Codex CLI | MCP008 bounded UV/tangent/PBR + four fixed passes；MCP009 receipt含真实 appearance/readback |
| Quality/Change/Version/Export | PASS MVP host golden path | MCP009 limited quality + approval/version + CAS-backed mvp-glb；pixel similarity/human gate NOT_RUN |
| MCP010A authority/dev activation | DONE / DESKTOP ATTEMPT 1 FAIL RETAINED / ATTEMPT 2 PASS | 第一次完整重启只见 17 个只读工具，失败 receipt 保留；修复后第二次完整重启观察到 30 个工具、Runtime `Ready`、`doctor` ready、临时 `project_create`/readback 和相同 build cohort，成功 receipt 已保存 |
| MCP010B V2 structural truth | BLOCKED/DEFERRED（source Gate PASS；Darwin OS memory hard cap NOT_RUN） | V2 geometry/readback、Skill integrity、Worker isolation、restore hardening 和 closed GLB profile已通过；不把受限预算/peak-RSS后验门当成OS预防式硬上限 |
| MCP010C fixed render/compare/review | IN_PROGRESS / PASS_WITH_UNRUN_VISUAL_GATES | 当前源码 59 contracts、20 read + 16 opt-in write；512×512 perspective/z-buffer、九 AOV、candidate-bound comparison、MCP image block、Codex/human review、deterministic raw stdio 和真实 Codex CLI 六 turn/32-call transport PASS；Viewer compare source implementation 与本地 IPC/build tests PASS；likeness threshold `FAIL_QUALITY_TARGET_NOT_MET`；packaged/current-cohort Viewer/C、独立人评、PBR/纹理、export/restart hash、HQ_360 NOT_RUN/BLOCKED |
| signed/notarized packaged Desktop | BLOCKED / NOT_RUN | MCP013；历史 codesign 为 `errSecInternalComponent` |
| IDE/其他 MCP Client/official transport conformance | OPTIONAL_NOT_IN_SCOPE | 不阻塞个人 MVP |

## 5. 当前任务：FGC-MCP010C（in_progress）

本轮新增 V1 AppearanceProgram@1 兼容基线：材质区、固定渲染元数据、UV/tangent 和 GLB 回读通过，但 limited aspect proxy 为 0.4662 < 0.55；候选已拒绝，未创建 version/export。它不能被升级解释为 V2 PBR 或视觉质量证据；详见 `docs/evidence/mcp010b/real-reference-v1-appearance-baseline.json`。

用户已明确领取 FGC-MCP010C。当前 C 已完成 source-focused implementation，并有真实 Codex CLI C transport receipt：固定 renderer、九 AOV、reference mask/metrics、MCP image block 和 Codex/human review。Viewer compare source implementation 已通过本地 IPC/构建测试；后续只补 C 的 packaged/current-cohort Viewer/人工评分/determinism/export-restart evidence；不得在 C 中接入 010D 高细节 Operator、Manifold、010E AssetPack/纹理或远程服务：

1. V1 `GeometryProgram@1` / `ArtifactReadback@1` 是 MCP007–009 的过渡兼容路径：历史 candidate/version 不迁移、不改写；在 `AppearanceProgram@2` 到位前，现有 MVP appearance golden path 仍可显式使用 V1，但不得把它写成 MCP010B 的 V2 高质量结果；
2. V2 必须有封闭 operator 参数 Schema、真实 DAG inputs、米/弧度单位、显式 Part outputs、operator catalog hash 和完整预算；`GeometryProgram@2.project_id` 必须与外层 `geometry_prepare.project_id` 完全相同，坐标范围为 ±10 m、dimension/height 不超过 10 m、radius/radii 不超过 5 m；
3. Runtime 必须读取 GLB JSON/BIN/accessor，而非相信 compiler extras；invalid index、non-finite、退化面、boundary/non-manifold、winding、Part/Material/source coverage、UV 和 tangent 失败必须 fail closed；
4. sphere 极点、cylinder 端盖与椭球 normal 的现有问题必须修复；不得继续写硬编码 `passed`；
5. 当前 registry/Bundle 不得因文档或 planned operator 宣称 active；不存在的 operator 必须保持 `partial/unavailable + missing_operator_ids`；
6. 只可新增受控 Schema、Rust worker/Runtime/MCP tests、first-party V2 catalog/evidence 和调用指引；Manifold/xatlas/mikktspace/Validator adoption、CC0 资产下载、纹理包、远程服务和可执行插件都不属于 010B；
7. 退出前完成 schema/negative → worker → Runtime/MCP → focused → aggregate 的适用 Gate，并将 PASS、FAIL、BLOCKED、NOT_RUN 分开写入 `docs/evidence/mcp010b/manifest.json`。V2 动态工具数量、真实参考视觉相似度和 360°一律不提前宣称。

当前 worktree 的 V2 路径已通过结构验证：`operator_catalog_get`、`forgecad://operators/catalog` 与 capability digest 一致；Codex 只能把 `operator_catalog_get` 返回的 digest 填入无 hash draft，再调用 `geometry_program_hash` 取得 compiler-owned `canonical_sha256`，最后调用 `geometry_prepare`。hash 工具不编译、不创建 candidate/Job，也不写 Store/CAS；它是 resource 的可调用镜像而非第二套 catalog 真值。catalog 节点仍是 closed leaf `primitive@2`，但真实图边是 `part_outputs[].input_node_ids` 的有序 semantic-Part sink：每个 source 必须恰好消费一次并保留逐 source readback binding。V2 target-project binding、10 m/5 m physical envelope、`ArtifactReadback@2` integrity、candidate-bound reread、confirm-time revalidation 和 Runtime-derived Skill availability 都有 source-built Gate。Codex/Luna 仍必须按 [Codex Geometry V2 工作流](CODEX_GEOMETRY_V2_WORKFLOW.md) 只以 JSON/BIN/accessor readback 判定结构硬门；未通过或未运行时不 confirm。

本次已运行 MCP010B structural Gate 作为 C 前置事实；C 新增的 `script/test_mcp010c.sh` 已通过 59-contract checker、Worker renderer、Runtime candidate-bound review unit、MCP existing suite、raw stdio compare/review/image-block/determinism Gate。真实 Codex CLI C 还在同 cohort 隔离 Runtime 中完成六 turn/32 MCP 调用，证据见 `docs/evidence/mcp010c/real-codex-cli-c-attempt13.json`；真实机器人结果为 `QUALITY_TARGET_NOT_MET`，未确认 candidate、未写入用户持久数据。Viewer compare source implementation 已通过本地 IPC/构建测试；packaged/current-cohort Viewer/C、独立人评、PBR/纹理、export/restart hash 和 HQ_360保持 `NOT_RUN/BLOCKED`。

### 当前 MCP010B 的 V2 authoring 已补齐，但任务未完成

`geometry_program_hash` 现在是默认只读 MCP 工具：输入 `GeometryProgramHashRequest@1`（严格、无 `canonical_sha256` 的 `GeometryProgram@2` draft），输出 `GeometryProgramHashResult@1`（Runtime/Worker-owned canonical hash、catalog hash、schema 和 `validation_status=passed`）。hash 工具对未知字段、V1、预填 hash 和 catalog mismatch fail closed；把返回 hash 填回 V2 program 后，`geometry_prepare` 再对 outer target project mismatch fail closed，且拒绝发生在编译/持久化之前。`operator_catalog_get` 是同一 Runtime-owned `OperatorCatalog@1` 的可调用读取面，必须与 resource/capability/artifact/readback digest 相等。两项能力和固定同级 Worker process isolation 已通过 raw/真实 Codex V2 structural Gate；它们不证明 Darwin 512 MiB OS 总内存硬门，也不打开后续 C–F 能力。

MCP010A 的文档/安装 Gate 仍是历史 PASS；MCP010B 的当前 `d9c23b…ac0bd` Dev.app evidence（install/ad-hoc package verify/isolated Ready/project/V2 raw/real Codex CLI structural probe/live Desktop structural activation）已写入其 manifest，不能借用 010A 或 MCP007–009 的历史 PASS。live activation 的 mismatch attempt 与成功 receipt 均保留；成功只覆盖结构工具链，不替代视觉、PBR 或后续 C–F 门。

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
