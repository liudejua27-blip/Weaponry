# ForgeCAD 当前原子任务索引

版本：2026-08-10
状态：唯一任务状态表；MVP host golden path 与 FGC-MCP010A 已收口；FGC-MCP010B 结构实现已通过、Darwin OS 总内存硬门 deferred；FGC-MCP010C in_progress，MCP010D–F blocked

## 1. 状态规则

状态只允许 `ready | in_progress | blocked | done | superseded`。同一时刻最多一个 `in_progress`；用户启动 Goal 后，Luna 才把唯一 `ready` 项改为 `in_progress`。依赖未完成时保持 `blocked`。

历史 evidence 的状态描述当时验收范围；任务索引描述当前权威范围。改变范围时不得改写原始 receipt，只能说明“现阶段退出条件”和仍留给后续任务的 Gate。

## 2. 当前任务链

| Task ID | 状态 | 依赖 | 当前原子结果 |
|---|---|---|---|
| FGC-MCP000 | done | 无 | ADR-0025、权威链、重置和执行任务链 |
| FGC-MCP001 | done | MCP000 | 可恢复硬切；新 Viewer/Runtime/contracts 骨架 |
| FGC-MCP002 | done | MCP001 | SQLite V1/CAS、Runtime OS 文件锁单写者、authenticated IPC |
| FGC-MCP003 | done | MCP002 | MCP stdio/resources/只读 tools；Codex Desktop/CLI P0 只读宿主证据 |
| FGC-MCP004 | done | MCP003 | 单用户事务基座：candidate/Job/approval/confirm/reject/restore/diagnostic export、MCP 内置 Runtime supervisor、一次有界 restart、Codex CLI diagnostic write、Viewer read model |
| FGC-MCP005 | done | MCP004 | 真实 Codex PNG/JPEG 附件字节 → CAS → `ReferenceEvidence@1`；CLI E2E PASS |
| FGC-MCP006 | done | MCP005 | MVP typed design/geometry/appearance 合同 + 10 个 first-party 声明式 Skill Bundle |
| FGC-MCP007 | done | MCP006 | 有界多 Part 硬表面机器人几何、Assembly/Part/source-map、GLB readback、Viewer read model |
| FGC-MCP008 | done | MCP007 | bounded UV/tangent/PBR、固定 beauty/silhouette/normal/part-ID、真实 GLB Viewer |
| FGC-MCP009 | done | MCP008 | limited quality projection、stable-Part `change_prepare`、拒绝/批准/版本/restore、CAS-backed MVP GLB export；功能核心收口 |
| FGC-MCP010A | done | MCP009 | 权威重排、可恢复旧代码清理、同 revision 用户级开发 App 激活、真实 Codex capability/build-hash Gate；Desktop attempt 1 FAIL 保留，attempt 2 PASS（30 工具、Ready、cohort match、临时项目 readback） |
| FGC-MCP010B | blocked | MCP010A | V2 geometry/readback/Worker isolation source Gate 已通过；Darwin 512 MiB OS 总内存硬门 deferred 为 NOT_RUN，不阻塞当前 C source implementation；历史 package/live receipts 保留 |

最新 d9c23b…ac0bd Skill-overlay Dev.app 已完成 package/raw/real-Codex V2 structural 子门；Bundle 知识现在明确区分视觉质量停止与用户明确批准的 `STRUCTURAL_BLOCKOUT` MVP 路径。该包已完成用户完整 Desktop restart，并通过 live Desktop structural activation；仍不得把它写成视觉/PBR能力。
| FGC-MCP010C | in_progress | MCP010B | 固定 512×512 perspective/z-buffer renderer、九 AOV、candidate-bound reference comparison、Codex/human review 与 MCP image block；source Gate PASS_WITH_UNRUN_VISUAL_GATES；默认 camera auto-fit 与视觉指标 CAS round-trip 修复已通过 raw source regression；Viewer compare source implementation/local IPC-build tests PASS；真实 Codex CLI 已完成六 turn/32-call C transport；机器人 likeness threshold 仍 FAIL_QUALITY_TARGET_NOT_MET |
| FGC-MCP010D | blocked | MCP010C | 受限高细节 Operator、first-party Skill 0.2 及 Manifold 隔离采用 |
| FGC-MCP010E | blocked | MCP010D | first-party 离线硬表面 AssetPack、UV/tangent/PBR/纹理 provenance |
| FGC-MCP010F | blocked | MCP010E | Viewer compare/selection/explosion、真实机器人闭环、partial/360 人工门 |
| FGC-MCP011 | blocked | MCP010F | Job checkpoint/并发/崩溃恢复/配额/GC/全局性能 |
| FGC-MCP012 | blocked | MCP011 | 第三方 Skill 生命周期、外部项目深度治理、分发签名/撤销 |
| FGC-MCP013 | blocked | MCP012 | Developer ID/notarization、clean install、升级回滚、packaged Desktop/CLI、跨类别真人门 |

当前领取任务：

```text
`FGC-MCP010C`：实现固定 renderer、九 AOV、参考比较、Codex/human visual review 和 MCP image block。合成/raw Gate 与首次真实机器人 reference→compare→review transport 已通过；默认 camera auto-fit 与视觉指标 CAS round-trip 的最新 raw source regression 也已通过（IoU 0.6623，仍 `QUALITY_TARGET_NOT_MET`）；Viewer compare source implementation/local IPC-build tests 也已通过；packaged/current-cohort Viewer/C、PBR/纹理、export/restart hash 和完整 360°仍必须保持独立状态。

补充当前事实：Dev.app packaged C 的安装/包验证/隔离探针、九 AOV raw renderer 和 Codex CLI compare/review transport 已通过；packaged Viewer UI、真实人评、PBR/纹理、export/restart hash 和 360°仍独立保持 `NOT_RUN/BLOCKED`。
```

010A 的旧代码清理与恢复 Gate 已 PASS：旧 Provider/Agent/standalone Host 入口、旧评估和孤儿运行残留已移除或隔离，两份 Host receipt 仅作为 `SUPERSEDED` 历史归档，用户 `output/`、`WushenForgeLibrary`、Runtime V1 与 Codex 历史均保留。MCP010A 的 30-tool Desktop receipt、MCP010B 的 3c/f488 Dev.app receipts和 884/896-triangle structural probes均为历史，原样保留。当前 MCP010B source 有 52 contracts（44 历史 + 8；含 `GeometryQualityReport@2`、`GeometryCandidateEvidence@1`），并已通过 V2 geometry/readback、Skill integrity、Worker isolation、MCP004/MCP007/MCP008/MCP009 回归、V2 restore hardening 和 closed GLB profile focused Gate；Darwin 512 MiB OS 总内存硬门仍 deferred/NOT_RUN。当前 MCP010C source 新增 7 个合同，总计 59；`script/test_mcp010c.sh` 已通过固定 512×512 perspective/z-buffer renderer、九 AOV、candidate-bound reference comparison、MCP image block、Codex/human review 和确定性 raw stdio Gate；真实 Codex CLI 另完成六 turn/32-call C transport，receipt 为 `docs/evidence/mcp010c/real-codex-cli-c-attempt13.json`。C 的 synthetic/reference/CLI structural evidence 不等于用户机器人 likeness 或高质量视觉 PASS；Viewer compare、packaged C、真实用户评分、PBR/纹理、export/restart hash 和 HQ_360 仍未运行，证据账本位于 `docs/evidence/mcp010c/manifest.json`。

## 3. MCP004 为什么现在可以 done

MCP004 的 MVP 责任是提供后续 3D 能力可复用的单用户事务与生命周期基座，不再把生产签名、真实附件、GLB exporter 和 packaged Desktop write 塞入同一前置任务。当前证据已覆盖：

- Runtime/IPC candidate prepare、quality/approval-bound confirm/reject、idempotency、stale/hash/expiry fail-closed；
- restore-as-new-version 和 path-free diagnostic manifest export；
- `forgecad-mcp` 拥有 stdio、异步启动/连接 Runtime、Runtime 缺失或崩溃时 stdio 继续、一次有界 restart；MCP010A 已补齐共享 Runtime 的启动选主、适配器退出存活和异常客户端隔离回归并通过最终源码 Gate；
- OS 文件锁单写者、进程退出自动释放、第二 Runtime `RUNTIME_BUSY`；
- 默认只读和显式 opt-in write 边界；
- 真实 Codex CLI diagnostic write E2E 与 Viewer authenticated read model；
- `npm run release:mcp004` PASS。

`docs/evidence/mcp004/manifest.json` 保留当时 `in_progress` 的历史现场与 `BLOCKED/NOT_RUN`，不篡改原始证据。未完成项已经转移：reference import 属于 MCP005，几何/外观/质量/GLB 属于 MCP007–009，distribution signing/packaged Desktop 属于 MCP013。这个范围调整不把它们写成 PASS。

## 4. MVP 任务退出条件

### FGC-MCP005 — Reference Intake（已完成）

Owned：Reference Schema、图片 admission、Runtime/CAS、MCP tool/probe、MCP005 evidence。

必须：

- `reference_import` 和 `reference_get` 使用公开 Schema；
- P0 只接受 PNG/JPEG，有限 byte/pixel/dimension/frame/decode memory；
- canonicalize、authorized root、symlink/目录/设备文件/MIME/魔数检查；
- CAS hash 与真实源字节一致，永久状态丢弃绝对路径；
- 真实 Codex CLI attachment-byte E2E；Desktop 不可传时记录 unavailable；
- success + truncated/oversize/decompression-bomb/path escape/symlink/hash mismatch tests；
- 更新 capabilities、合同、用户文档和 `docs/evidence/mcp005/manifest.json`。

已完成证据：PNG/JPEG Runtime admission、authorized-root/outside-root/symlink/hash/MIME negative tests、CAS readback、authenticated MCP adapter、真实 Codex CLI `project_create → reference_import → reference_get`。Codex Desktop 当前 bridge 仍记录 `NOT_RUN / unavailable`，不影响 CLI 任务退出；原图路径和字节没有进入仓库、DB、receipt 或日志。MCP010C 另有一份隔离真实机器人九-AOV compare/review receipt，但当前视觉阈值未通过。

禁止：Geometry/Render、远程 image-to-3D、Blender/Python、将图片复制进 Git。

### FGC-MCP006 — Contracts + MVP Skills

Owned：SubjectProfile/RepresentationPlan/AssemblyGraph/GeometryProgram/AppearanceProgram/RecipePlan Schema，first-party Skill 包、validator/benchmark、Registry 开发模式。

必须：

- 10 个核心 Skill 的知识、Schema、Recipe、operator lock、validator、fixture、LICENSE/NOTICE、SPDX SBOM、provenance；
- first-party canonical hash/trust root；分发签名延后但完整性不可省略；
- DAG cycle、unknown operator、错误单位、non-finite、预算、license/SBOM/hash drift fail closed；
- Bundle 不携带 executable、不访问网络/环境变量/任意路径；
- 同输入 canonical plan hash 可重复。

已完成：44 个 contracts schema；Runtime 内置 development-only Skill Registry 的十个 first-party Skill；`skill_list`/`skill_get` 与 `forgecad://skills/{skill_id}/{version}` 只读资源；十个独立 `bundles/<skill_id>/0.1.0` 标准目录；每项 Recipe、operator lock、validator subset、synthetic adversarial fixtures、benchmark receipt、LICENSE/NOTICE、SPDX SBOM、provenance、development trust manifest 和明确延期签名占位；registry/bundle hash、DAG cycle、单位、finite、预算、未知 operator、路径/脚本/网络 capability 等 fail-closed Gate；真实 Codex CLI `capabilities_get → skill_list → skill_get` 和 Runtime/MCP focused tests。MCP006 不把 Skill metadata 误写成 3D 结果，后续 geometry/appearance consumer 已分别在 MCP007–009 通过 focused Gate。

### FGC-MCP007 — Geometry Vertical Slice

Owned：geometry contracts/core/worker、Runtime compile orchestration、GLB lowering/readback、Viewer artifact display、MCP007 evidence。

MVP 当前退出 Gate 只要求已经实现的 bounded primitive/transform 子集；其余 Operator
不得被 Skill metadata 伪装成可执行能力：

- 当前 allowlist：`forgecad.geometry.primitive@1` 的 box/cylinder/sphere 与有界 transform；
- 声明式但延期：profile/extrude/revolve/sweep/loft/mirror/array/bounded boolean/bevel/hard-surface macros；
- 机器人由多个稳定语义 Part 组成，不是单 mesh 占位；
- 真实非空 mesh/GLB，finite/index/normal/degenerate/manifold/budget Gate；
- Part/MaterialZone/source Operator lineage strict readback；
- deterministic fixture、恶意参数、timeout、Worker crash 和 no-version-on-failure；
- Viewer 显示同一 candidate hash；提交 GLB/readback/wireframe/part-ID evidence。

已完成（MCP007 evidence）：product-owned geometry worker library/binary 只接受 canonical `GeometryProgram@1`，当前实现 box/cylinder/sphere、有限预算、finite/unique ID/allowlist 校验和确定性 glTF 2.0 GLB lowering；Runtime 在候选事务中写入 GLB CAS、生成 `GeometryQualityReport@1`、返回 `ArtifactReadback@1`，并提供 authenticated `artifact_readback_get`。机器人 fixture 有 14 个语义部件、516 triangles；worker fixture 有 3 部件、332 triangles，重复输出 hash 相同。Viewer read model 通过 authenticated IPC 读取候选和 artifact readback 元数据，不启动 Runtime、不写数据库。focused worker/Runtime/MCP/Viewer Gate 与 `npm run mcp007:test` PASS；真实 Codex CLI 已使用用户授权 PNG 完成同一 typed geometry slice，14 parts/516 triangles/validator passed，见 `docs/evidence/mcp007/codex-cli-geometry.json`。MCP009 真实 Codex receipt 另证明该 geometry slice 可继续进入 Appearance/Render/Quality/Confirm/Export。当前实现刻意没有宣称 profile/extrude/revolve/sweep/loft/boolean/bevel 全量、参考相似度或通用质量；后续任务只在真实合同下扩展。

### FGC-MCP008 — Appearance + Render + Viewer（done）

Owned：UV/tangent/PBR contracts/compiler、render worker、Viewer 3D/part/material UI、MCP008 evidence。

已通过（功能核心）：

- UV unwrap/pack、MikkTSpace、glTF metallic-roughness + AO/Normal/Emissive；
- 白外壳/黑机械/橙 emissive typed MaterialZone；
- GLB validator 0 errors、Runtime strict readback；
- Viewer 只消费 Runtime artifact，选择/隔离不写版本；
- Viewer 关闭时 headless beauty/silhouette/normal/part-ID 仍生成；
- 固定 camera/light/resolution/renderer version/hash receipt。

证据：`docs/evidence/mcp008/manifest.json`、`worker-fixture.json`，以及 `npm run mcp008:test`；真实 Codex appearance/readback 由 `docs/evidence/mcp009/codex-cli-appearance-export.json` 的十二调用 receipt 覆盖。限制：没有把合成 fixture、单张 beauty 或 Skill metadata 写成参考相似度 PASS；packaged Viewer、像素/区域指标和真人评分仍是 `NOT_RUN`。

### FGC-MCP009 — MVP Golden Path（功能核心 done）

Owned：reference compare/Quality、SemanticChangeSet、production GLB export、Codex probe、完整 evidence pack、MVP 文档。

必须：

- `quality_get` 输出 Runtime-owned QualityReport；当前 reference compare 是明确标记 `limited` 的 aspect-ratio evidence，不冒充 silhouette IoU/landmark/region 完成；
- Codex visual review 绑定具体 render/pass/region，不能覆盖硬门；
- `change_prepare` 对稳定 Part ID 做一次有界局部修改，要求 base version 和新 typed programs；MVP 不实现通用 mesh-delta/DAG 复用；
- reject 不写版本；approve 只写一个不可变子版本；restore 创建新版本；
- export 绑定 confirmed version/artifact/quality lineage，`mvp-glb` 返回 CAS GLB hash 和 manifest receipt；MVP 不写任意本机路径；
- Runtime focused golden path、reject/restore/idempotency 和 24 个 Runtime tests、16 个 MCP tests 已通过；真实 Codex CLI 已使用授权图片完成十二调用 `project_create → reference_import → geometry_prepare → artifact_readback_get → appearance_prepare → artifact_readback_get → quality_get → candidate_confirm → version_list → export_prepare → export_confirm → version_list`，并取得 geometry/appearance/readback/fixed-render/quality/CAS GLB receipt；
- 因此可以声明“单用户 MVP host golden path 可供开发评估”，但不能宣称通用高质量、像素级参考相似度、真人验收或 packaged release。`change_prepare`、restore、restart 同 hash 和 Viewer 交互仍是独立后续 host Gate。

证据：`docs/evidence/mcp009/manifest.json`、`codex-cli-appearance-export.json`。下一步不是继续堆基础设施；若继续 Goal，先做 Viewer 同 hash、局部修改/restore 的真实 host 验证，再做独立真人评审。不得把 CAS receipt 或 aspect-ratio limited 比较升级为产品质量结论。

## 5. MCP010 质量升级与后续退出边界

MCP010A–F 的详细要求见 `MCP010_HIGH_QUALITY_HARD_SURFACE_PLAN.md`。它们不改写 MCP005–009 的历史 PASS：当前单图目标最多是 `PARTIAL_VISIBLE_VIEW_PASS`，补齐五张全身参考前 `HQ_360_PASS=BLOCKED_REFERENCE_COVERAGE`。Job checkpoint/GC/通用并发属于 MCP011；通用第三方 Skill/AssetPack 安装、publisher、签名和撤销属于 MCP012；外部分发、自动安装、Developer ID/notarization、filesystem/package export、packaged E2E 和跨类别质量宣传属于 MCP013。

## 6. 每任务证据

每个任务新建 `docs/evidence/mcpXXX/manifest.json`；MCP010 原子任务使用 `docs/evidence/mcp010a/` 至 `mcp010f/`。至少记录 worktree/base、命令和 exit code、artifact/contract/dependency hash、focused/aggregate/real Codex、FAIL/BLOCKED/NOT_RUN、无绝对路径/secret 检查。视觉任务另含 ReferenceEvidence、program、GLB/readback、RenderSet、QualityReport 和人工评分。

旧 U/C/K/F/E/VP/U004 任务均为 `superseded` 或历史，不重新进入依赖链。
