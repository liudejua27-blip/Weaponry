# ForgeCAD MVP 工具、Skill 与外部项目目录

版本：2026-08-09
状态：MVP 功能核心目录；当前仍为 17 read + 13 write、十个 Skill `0.1.0`；MCP010 目标另列 planned/unavailable

本文是 Luna 执行 Goal 时的“能调用什么、何时调用、什么不能声称”的单一索引。它不是新的运行时配置，也不允许绕过 MCP 合同。工具实现仍以 Rust source 和 JSON Schema 为权威；本文只提供可读的路线图和验收边界。

## 1. MVP 运行边界

```text
Codex Desktop / CLI
        │ MCP stdio
        ▼
forgecad-mcp  ── authenticated local IPC ── forgecad-runtime
                                                │
                                                ├─ SQLite V1 + CAS（唯一写者）
                                                └─ bounded typed geometry/appearance/render
        ▲
        │ read-only IPC
ForgeCAD Viewer（可选）
```

- 默认连接暴露 17 个只读工具；只有 authenticated IPC、Runtime handoff 和 `FORGECAD_MCP_ENABLE_MCP004_WRITES=1` 同时满足时，才暴露完整 30 个工具（17 read + 13 write）。
- `forgecad-mcp` 不打开 SQLite/CAS，不执行模型调用，不接受任意 Python、JavaScript、shell、URL 或未授权路径。
- Runtime 启动前取得 `runtime.writer.lock`；MVP 不使用 TTL lease、heartbeat、broker、远程 transport 或插件市场。
- Viewer 只读 Runtime projection；关闭 Viewer 不删除已确认数据，但 MVP 不承诺 Codex 断线后未完成 Job 继续。
- `functional-core PASS` 只证明 focused 本地实现；当前已有真实 Codex CLI 十二调用 host golden-path receipt。真人视觉评分、外部分发和签名仍必须有独立 receipt。

## 2. 只读工具（默认可见，17 个）

| 工具 | 用途 | 当前 MVP 证据/限制 |
|---|---|---|
| `artifact_readback_get` | 读取候选绑定的 GLB header、Part、triangle、UV/tangent、PBR readback | MCP007/008 focused PASS；不返回任意文件路径 |
| `candidate_get` | 读取 candidate、hash、Job、quality 摘要 | 只读；未确认 candidate 可回收 |
| `capabilities_get` | 读取 Runtime/MCP/Worker/Skill 能力和 limitation | 必须在写入前调用；不以空字段伪装能力 |
| `doctor` | 读取 bounded health/contract/lock 诊断 | 不启动 fixture、不 confirm、不签名 |
| `job_events_read` | 读取 durable Job events | MVP 支持读取/取消；checkpoint 续跑属 MCP011 |
| `job_get` | 读取 Job 状态 | 非终态重启可转 typed failure |
| `project_get` | 读取项目元数据和 head | 不创建项目 |
| `project_list` | 列出当前 Runtime 项目 | 不读取旧 Library |
| `reference_get` | 读取 ReferenceEvidence hash/MIME/尺寸/授权 | 不返回原始绝对路径或图片字节 |
| `quality_get` | 读取 Runtime-owned quality report | hard checks PASS；reference compare 目前只返回明确 `limited` aspect evidence |
| `selection_get` | 读取 Viewer 临时 selection | ephemeral，不是版本真值；当前可为 unavailable |
| `runtime_status` | 读取 Runtime 生命周期 | `Starting/Ready/Degraded/Restarting/Busy` 只做状态投影 |
| `skill_get` | 读取 first-party Skill manifest | development-only metadata，不等于结果质量 |
| `skill_list` | 列出 10 个 first-party Skill | 不安装第三方 Bundle |
| `snapshot_get` | 读取 `ActiveDesignSnapshot` | 单一当前投影，不复制资产状态 |
| `version_diff` | 读取两个不可变版本的结构化差异 | MCP009 focused PASS；不提供通用 mesh diff |
| `version_list` | 列出项目版本 DAG | 历史不可变；restore 创建新子版本 |

只读工具必须带 `readOnlyHint=true`、`destructiveHint=false`、`idempotentHint=true`、`openWorldHint=false`。如果 Runtime 不可用，调用返回 `RUNTIME_UNAVAILABLE`；Runtime 已连接但拒绝请求时返回 `INVALID_INPUT`、`STORE_ERROR`、`RUNTIME_BUSY` 等 typed code。不能因为 stdio initialize 成功就声称 Runtime ready。

## 3. 写工具（显式 opt-in 可见，13 个）

### MCP004：事务基座（9 个）

| 工具 | 用途 | 永久版本 |
|---|---|---|
| `project_create` | 创建项目元数据 | 是项目记录，但不创建资产版本 |
| `candidate_prepare` | 准备 diagnostic 或已入 CAS 的 typed candidate | 否 |
| `candidate_confirm` | 对已批准 candidate 创建版本 | 是；hash/head/quality/approval/idempotency 必须重新校验 |
| `candidate_reject` | 拒绝 candidate | 否 |
| `restore_prepare` | 以历史 confirmed version 为内容准备新 candidate | 否 |
| `restore_confirm` | 确认 restore candidate | 是新子版本，不改写历史 |
| `export_prepare` | 准备 path-free manifest 或 `glb/mvp-glb` | 否 |
| `export_confirm` | 确认导出并生成 CAS receipt | 不写任意本机路径；返回 `output_sha256` |
| `job_cancel` | 请求取消 Job | 否 |

### MCP005–MCP009：3D vertical slice（各 1 个）

| 工具 | 任务 | 当前行为 |
|---|---|---|
| `reference_import` | MCP005 | 仅 PNG/JPEG；真实字节经授权 root/inline admission 进入 CAS，返回 `ReferenceEvidence@1` |
| `geometry_prepare` | MCP007 | 仅 canonical `GeometryProgram@1`；当前 product-owned primitive allowlist 为 box/cylinder/sphere，输出多 Part GLB/readback |
| `appearance_prepare` | MCP008 | 绑定同一 geometry candidate；输出 bounded UV/tangent/PBR MaterialZone 和四个 fixed PNG pass |
| `change_prepare` | MCP009 | 需要当前 `base_version_id`、稳定 `part_id`、allowlisted operation 和 typed programs；生成新 candidate，不改历史 |

所有写工具都声明 `readOnlyHint=false`。需要用户批准的 `candidate_confirm`、`restore_confirm`、`export_confirm` 以及由写流程生成的永久版本都必须绑定 approval context；MVP receipt 是宿主流程证据，不是密码学人类签名。

### 3.1 MCP010C 目标工具（当前 unavailable）

| 工具 | 目标类型 | 目标行为 |
|---|---|---|
| `render_pass_get` | read | 返回已经持久化、hash-bound 的 PNG image block，不隐式生成 render |
| `reference_compare_prepare` | write/temporary | 生成 camera/mask/metrics/diff，不创建版本 |
| `visual_review_submit` | write/evidence | 保存绑定 pass/region/candidate hash 的 Codex typed issue |
| `human_visual_review_submit` | write/evidence + confirmation | 保存用户评分；不作为密码学身份认证 |

`quality_get` 是既有只读工具，只在 `QualityReport@2` producer/validator/negative tests 完成后升级语义。C 完成前当前数量保持 17 read + 13 write；全部 Gate 通过后目标才是 18 read + 16 write。不得在 tool list 先暴露空实现。

## 4. Luna 推荐调用顺序

```text
capabilities_get
→ project_create
→ reference_import（真实用户授权附件）
→ reference_get
→ skill_list / skill_get（选择 first-party Bundle）
→ geometry_prepare
→ artifact_readback_get / candidate_get
→ appearance_prepare
→ artifact_readback_get
→ quality_get
→ candidate_reject（验证拒绝不写版本）
→ change_prepare（稳定 Part 的一次有界修改）
→ candidate_confirm（用户批准）
→ version_list / version_diff
→ restore_prepare → restore_confirm
→ export_prepare(format=glb, profile=mvp-glb)
→ export_confirm
```

每一步都记录 `project_id`、candidate/version/artifact hash、Job 状态、MIME/size、quality limitation 和 receipt。任何一步失败都停止写链路并记录 `FAIL`、`BLOCKED` 或 `NOT_RUN`，不要自动退回旧 Provider 或手工 GLB。

## 5. First-party Skill Bundle（10 个）

Skill Bundle 是声明式 metadata + typed Recipe；Runtime 只解析已注册 Operator，Bundle 自身不携带可执行脚本。当前 Registry 为 `development-only`，每个 Bundle 均有 Schema、Recipe、operator lock、validator、fixture、benchmark receipt、LICENSE/NOTICE、SPDX SBOM、provenance 和 canonical trust manifest。

| Skill | 当前 consumer | MVP 作用 | 限制 |
|---|---|---|---|
| `reference-intake` | MCP005/006 | 参考 hash/claims 边界；保留 staged detail inventory、可见/遮挡区和 unknowns | 不执行图片理解，不调用模型；Codex 负责语义判断 |
| `subject-profile` | MCP006/009 | typed subject/profile 草案、每区域 confidence 与“不确定而非猜测”记录 | 由 Codex 产生语义，Runtime 只校验范围和 hash |
| `semantic-assembly` | MCP006/007 | 稳定 Part/Assembly 图 | 不生成任意 mesh |
| `silhouette-blockout` | MCP007 | 有界 primitives blockout | 当前 compiler 只接受 box/cylinder/sphere |
| `hard-surface-detail` | MCP007/009 | panel/vent/joint 等声明式细节 | 未实现的 operator 必须 fail closed |
| `mesh-integrity` | MCP007/008 | finite/index/degenerate/readback 硬门 | 不是视觉相似度 |
| `uv-pbr` | MCP008 | UV/tangent/MaterialZone/glTF PBR | 无纹理烘焙、UDIM 或完整色彩管理 |
| `render-evidence` | MCP008 | fixed camera 与 beauty/silhouette/normal/part-ID | 无跨 GPU/完整 AOV 保证 |
| `reference-compare` | MCP009 | quality report 的有限参考元数据比较；为未来 silhouette/landmark/region 留 typed evidence 位 | 当前仅 aspect-ratio `limited`，不把颜色/CSS preview 当相似度 |
| `local-edit-and-export` | MCP009 | stable-Part change、approval、CAS `mvp-glb` | 不支持通用 mesh delta 或任意路径导出 |

Skill metadata 的 operator ID 不等于当前全部 operator 已实现。当前可执行能力以 `geometry_prepare`/`appearance_prepare` 的 Runtime allowlist 和 `capabilities_get` 为准。

### 5.1 MCP010 目标 Skill 版本（当前不可用）

| 任务 | 目标版本 | 激活前置条件 |
|---|---|---|
| MCP010D | `reference-intake@0.2.0`、`silhouette-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`mesh-integrity@0.2.0` | V2 Schema、真实 Operator consumer、validator/benchmark/receipt |
| MCP010E | `uv-pbr@0.2.0`、`render-evidence@0.2.0`、`reference-compare@0.2.0` | AssetPack、UV/tangent/PBR/Render/metric producer 和逐资产 provenance |

其他 Skill 保持 `0.1.0`。AssetPack 是独立资产合同，不是第十一个可执行插件；缺少 operator/asset/benchmark 时目标 Bundle 必须返回 partial/unavailable。

## 6. GitHub/外部工具决策

本文引用 GitHub 只表示研究候选，不表示已安装。当前没有 `accepted` 第三方 3D compiler、UV、tangent 或 renderer dependency。

| 项目 | 状态 | 允许的下一步 | 禁止行为 |
|---|---|---|---|
| `image-rs/image` | approved-for-evaluation | 隔离图片 decoder benchmark | 未固定 revision 就改 lockfile |
| `gltf-rs/gltf` | approved-for-evaluation | GLB strict readback benchmark | 接受外部 URI/任意 buffer |
| `elalish/manifold` | approved-for-evaluation | bounded boolean worker benchmark | 直接把 FFI 变 Runtime 真值 |
| `jpcy/xatlas` | approved-for-evaluation | UV seam/overlap/determinism benchmark | 未审计就替换 product-owned UV |
| `gltf-rs/mikktspace` | approved-for-evaluation | tangent golden benchmark | 漂移时静默改变 PBR |
| `KhronosGroup/glTF-Validator` | approved-for-evaluation | 外部 GLB validator receipt | 用外部报告替代 Runtime readback |
| `donmccurdy/glTF-Transform` | approved-for-evaluation-as-dev-tool | dev-only inspect/optimize | Node 进程写 SQLite/CAS 真值 |
| `img2threejs/img2threejs` | approved-for-evaluation / first-party reimplementation | staged passes、detail inventory、per-region confidence、side-by-side compare | Apache-2.0；不安装其 Python/TypeScript/Three.js skill，不把 JS 变 Runtime 真值 |
| `javierbyte/img2css` | reference-only visualizer idea | bounded 低分辨率颜色/区域预览，帮助 Codex 形成材质区和轮廓草图 | BSD-3-Clause；不执行其 JS，不保存 CSS/base64，不进入 GeometryProgram |
| Blender / BlenderMCP / FreeCAD MCP / CadQuery | reference-only/rejected for MVP | 只学习交互/算法 | 任意 Python、socket、网络资产、`.blend` 真值 |
| TripoSR/Hunyuan3D/远程 image-to-3D | rejected for MVP | 另立 ADR 后再评估 | 下载权重、远程 Provider、绕过 typed compiler |

采用任何外部项目之前，Luna 必须新增 `docs/evidence/adoption/<project>/<full-revision>.yaml`，包含精确 revision、许可证文件 hash、transitive SBOM、恶意输入/资源测试、determinism benchmark、平台结果和 removal plan；只有 `approval: accepted` 才能改 lockfile 或打包。

## 7. 当前 MVP 状态和下一步

- `MCP005–MCP009 functional core`：focused tests/evidence PASS；可运行 `npm run mvp:functional-core`（包含 MCP005 本地 admission 回归；真实 Codex attachment probe 仍单独记录）。
- 真实 Codex MVP host golden path（参考附件 → geometry → appearance → quality → confirm → version → CAS export）：已由用户授权图片的 Codex CLI receipt 证明；MCP010A 另有第二次 Desktop 激活 Gate PASS。`reject → change → restore`、完整 Desktop 3D write、Viewer 同 hash、重启后的模型恢复和 packaged write 仍 `NOT_RUN/BLOCKED`，不能用 fixture 冒充。
- glTF Validator、像素级 silhouette/landmark/region、独立真人视觉评分、packaged Viewer、Developer ID/notarization：当前 `NOT_RUN/BLOCKED`，不属于本地 functional-core 命令的隐含 PASS。
- 用户已继续产品化 Goal；`FGC-MCP010A` 已完成真实 Desktop capability/build-hash Gate，`010B–F` 仍 blocked，必须由后续独立 Goal 领取；不先建设 heartbeat、broker、通用 pack installer 或插件市场。
