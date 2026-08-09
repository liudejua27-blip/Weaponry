# FGC-MCP010 高质量硬表面参考闭环计划

版本：2026-08-09
状态：`FGC-MCP010A in_progress`；`FGC-MCP010B`–`FGC-MCP010F blocked`
依赖：`FGC-MCP009 done（MVP host golden path）`

本文是 MCP010A–F 的唯一详细执行合同。它不改写 MCP005–009 的历史 evidence，也不把目标 Schema、工具、Skill、库或素材写成当前能力。

## 1. 目标和声明边界

把现有 primitive blockout 升级为 Codex 可驱动、可回读、可比较、可局部修改和可回退的首个白色硬表面机器人质量闭环：

```text
ReferenceEvidence
→ typed detail inventory
→ GeometryProgram/AppearanceProgram
→ mesh + UV/PBR + self-contained GLB
→ fixed RenderSet/AOV
→ reference metrics + typed visual review
→ user review/approval
→ immutable version → restore → CAS export
```

当前只有一张裁切腿脚、正面三分之四参考。因此：

- 本轨道首先验收可见视图，最高状态是 `PARTIAL_VISIBLE_VIEW_PASS`；
- 用户补充 front、back、left、right、rear-three-quarter 五张同设计全身参考之前，`HQ_360_PASS` 固定为 `BLOCKED_REFERENCE_COVERAGE`；
- 隐藏结构必须标记 `unknown/inferred`，不能以对称或想象伪装成参考事实；
- 本轨道不承诺骨骼动画、制造 CAD、工程安全、跨类别通用重建或公开发行。

## 2. 当前事实与目标分离

| 项目 | 当前事实 | MCP010 目标 |
|---|---|---|
| 合同 | 44 个 JSON Schema，Geometry/Appearance/Readback/Render/Quality `@1` | 按 B/C/E 分阶段加入 V2/新合同 |
| MCP | 17 read + 13 write | C 完成后计划为 18 read + 16 write |
| Skill | 十个 first-party `0.1.0` declarative Bundle | D/E 按真实 consumer 升级七个 `0.2.0` Bundle |
| 几何 | box/cylinder/sphere + limited transform | profile/revolve/sweep/loft/mirror/array/macros；boolean 有条件采用 |
| Render | 四个 bounded pass；reference compare limited | perspective/z-buffer 固定 renderer + 九 AOV +真实指标 |
| 材质 | factor-only bounded glTF PBR | first-party 离线 AssetPack、纹理、clearcoat/emissive strength |
| Viewer | 只读 GLB canvas/read model | compare/AOV/diff/Part/MaterialZone/isolate/explosion/a11y |

只有对应 producer、consumer、negative/focused/真实 Codex evidence 全部通过后，能力才能从 `planned/unavailable` 变为 `available`。

## 3. 原子任务链

### 3.1 FGC-MCP010A — 权威重排与开发激活

Owned：权威文档、文档 checker、用户级开发 App 构建/激活、原始 stdio/CLI/真实 Codex capability evidence。

必须：

1. 把任务索引重排为 010A–F；同一时刻只保留 010A `in_progress`；
2. 从同一源码 revision 构建 `forgecad-mcp`、`forgecad-runtime`、Worker 和 Viewer；
3. 安装到 `~/Applications/ForgeCAD Runtime Dev.app`，开发期只允许本机 ad-hoc 签名；
4. Codex 用户配置指向 App Resources 中的 `forgecad-mcp`，不再引用 `forgecad-mcp-host`；仓库配置不写 token、fixture data dir、用户名或用户绝对路径；
5. 原始 MCP/CLI Gate 通过后，由用户重启 Codex；真实调用证明 `capabilities_get`、临时 `project_create`、Runtime `Ready` 和 MCP/Runtime 相同 build hash。

退出：用户重启后的真实证据未观察到前保持 `in_progress`；不得领取 010B。ad-hoc 开发 App 不是 MCP013 的签名安装包。

当前进度：证据见 `docs/evidence/mcp010a/`。用户 Codex 配置已切换，第一次 Desktop 重启后的 live Gate 已实际运行并 `FAIL`，不是 `NOT_RUN`：仅发现 17 个只读工具、无 `project_create`，Runtime 实际不可用且没有项目写入；该历史 receipt 保持原样。显式 server environment write opt-in 已配置。共享 Runtime supervisor/IPC 修复已通过最终源码的 `script/test_mcp004.sh`（MCP 26/26 + lifecycle）和 `release:mvp`（Runtime 30/30、MCP 26/26、44 contracts、MCP005–009、Viewer/Tauri、docs/security）。短时 launcher flock 只用于启动选主，Runtime `runtime.writer.lock` 才是最终唯一写者；正常适配器退出不停止已经 Ready 的共享 Runtime，显式 shutdown/update 才停止。最终同 revision/cohort Dev.app 已按 `7a8fddf99c57893db93fe1bdd98ab65302bd890d191026495cbbc63ae4652064` 重建安装；ad-hoc deep-strict、`package:verify` 与隔离 package probe PASS。隔离 probe 协商 `2025-06-18`、观察到 `Ready` 与 build cohort match、完成隔离 `project_create`，且未触碰持久用户数据。Geometry Worker 与 MCP/Runtime/Viewer 同 cohort 打包，但尚未被 Runtime 作为独立子进程调用，不属于 010A 完成声明。用户第二次完整 Desktop 重启仍为 `NOT_RUN`，因此 010A 保持 `in_progress`，B–F 保持 `blocked`。

### 3.2 FGC-MCP010B — V2 合同与几何真值

Owned：`GeometryProgram@2`、`OperatorCatalog@1`、`ArtifactReadback@2`、GLB/accessor validator、primitive 修复及负向 fixture。

必须：

- `GeometryProgram@2` 按 `operator_id` 使用封闭参数 Schema，真实 DAG inputs，米/弧度单位，显式 Part outputs、operator catalog hash 和完整预算；
- 修复 sphere 极点退化、cylinder 端盖法线、ellipsoid 法线、UV/tangent 假 PASS；
- Runtime 遍历 GLB BIN/accessor，真实计算 index/non-finite/degenerate/boundary/non-manifold/winding/Part/Material/source coverage；
- 删除 hard-coded validator PASS；损坏 index、source map、hash、winding 或 UV 时 fail closed；
- `@1` 仅保留历史只读，不迁移或改写已确认版本。

预算：512 nodes、250k triangles、64 MiB candidate GLB、512 MiB Worker memory；单次编译目标 10 秒以内，但测量失败不能转移为 MCP011 全局性能实现。

### 3.3 FGC-MCP010C — 固定渲染与参考比较

Owned：`ReferenceViewSpec@1`、`CameraCalibration@1`、`RenderSet@2`、`ReferenceComparisonReport@1`、`VisualReviewReport@1`、`HumanVisualReviewReceipt@1`、`QualityReport@2` 和四个 MCP 工具变化。

Renderer 必须提供 512×512 perspective、真实 camera transform、z-buffer、确定性抗锯齿、固定 GGX 直接光和显式色彩管理；同一 candidate hash 输出：

1. beauty；
2. silhouette；
3. depth；
4. normal；
5. AO；
6. part-ID；
7. material-ID；
8. wireframe；
9. UV-stretch。

目标工具：

| 工具 | 类型 | 行为 |
|---|---|---|
| `render_pass_get` | read | 返回 hash-bound PNG image block；不生成新 render |
| `reference_compare_prepare` | write/temporary | 生成 camera、mask、metrics 和 diff，不创建版本 |
| `visual_review_submit` | write/evidence | 保存 Codex 对具体 pass/region 的 typed issue |
| `human_visual_review_submit` | write/evidence + confirmation | 保存用户评分，不作为密码学身份认证 |
| `quality_get` | existing read | 只读取 Runtime 已持久化且绑定当前 hash 的报告 |

当前工具数量在 C 完成前仍是 17 read + 13 write；C 全部门通过后目标才是 18 read + 16 write。

参考 mask 使用产品内确定性 border flood-fill/morphology；Codex 提交 normalized landmarks、region、visibility 和 unknown/inferred。每个 candidate 最多五轮 `silhouette → structure → form → material/surface → final` 修正；未达标返回 `QUALITY_TARGET_NOT_MET`，不能自动 confirm。

### 3.4 FGC-MCP010D — 受限高细节几何

Owned：真实 Operator consumer、Worker 隔离/预算、Operator catalog、geometry Skills `0.2.0` 和 Manifold adoption。

目标 Operator：

- `primitive@2`：rounded-box、正确 cylinder/sphere；
- `profile-extrude@1`、`profile-loft@1`、`revolve@1`、`tube-sweep@1`；
- `transform@2`、`mirror@1`、`array@1`；
- `boolean@1`：只允许同一 Part scope 的 union/difference；
- `panel@1`、`vent-array@1`、`joint-stack@1`；
- `part-output@1`：一个语义 Part 可由多个细节节点组成。

Manifold 固定目标为 v3.5.2，仅 C API 静态进入隔离 geometry worker，关闭 Python/JS binding、自动下载和不受控并行。采用前必须有 full revision、LICENSE hash、transitive SBOM、恶意输入/时间/内存/确定性/source-ID benchmark 和 removal plan；receipt 未 `accepted` 时 `boolean` 保持 unavailable，机器人使用分层 shell，不阻塞其他 Operator。

升级：`reference-intake@0.2.0`、`silhouette-blockout@0.2.0`、`hard-surface-detail@0.2.0`、`mesh-integrity@0.2.0`。只有所有 operator lock 均有真实 consumer 时才 active。

机器人需要稳定 head/visor/neck、chest/core/shoulder、arm/hand、pelvis/hip、thigh/knee/shin/ankle/foot，并包含可追踪 panel、vent、joint ring、cable、emissive housing；左右结构使用 mirror，不维护两套漂移参数。

### 3.5 FGC-MCP010E — 离线 AssetPack、UV 与 PBR

Owned：`MaterialPackManifest@1`、`MaterialDefinition@1`、`TextureSet@1`、`TextureBuildReceipt@1`、`AppearanceProgram@2`、first-party AssetPack、xatlas/mikktspace/glTF Validator adoption 与材质 Skills `0.2.0`。

`forgecad-hard-surface-robot@1.0.0` 必须是 AssetPack，不是 Skill，包含：白色 dielectric clearcoat、深灰喷涂金属、黑色阳极氧化金属、拉丝钢、工程塑料、关节橡胶、暖橙 emissive 和微划痕 normal/roughness 层。

实施期只允许 Codex 一次性下载以下免费 CC0 文件到本机 adoption cache，不调用 API：

- ambientCG `Metal010` 2K PNG；
- ambientCG `Plastic006` 2K PNG；
- Poly Haven `Studio Small 03` HDRI。

每个原文件记录 source URL/ID、retrieved_at、SHA-256、作者、SPDX、license text hash、通道/色彩空间和处理 Recipe。原 ZIP 不进入 Git；派生 `.forgecad-material-pack` 经 manifest 校验后进入开发 App Resources，Runtime 首次启动写 CAS，运行时永不联网。

UV/tangent/GLB 规则：

- xatlas 和 mikktspace 固定 revision，经 receipt accepted 后才加入；
- baseColor/emissive 为 sRGB，normal/metallic/roughness/AO 为 linear，normal 固定 OpenGL `+Y`；
- GLB 内嵌 PNG、禁止 external URI、按材质 hash 去重；candidate texture ≤64 MiB，export ≤128 MiB；
- 支持 ratified `KHR_materials_clearcoat`、`KHR_materials_emissive_strength`；KTX2/LOD/通用 pack installer 延后；
- glTF Validator 是开发 Gate，不能替代 Runtime readback。

升级 `uv-pbr@0.2.0`、`render-evidence@0.2.0`、`reference-compare@0.2.0`；AssetPack、Operator 和 benchmark 缺一时保持 partial/unavailable。

### 3.6 FGC-MCP010F — Viewer 与真实机器人闭环

Owned：reference/render split、overlay、flicker、diff heatmap、九 AOV、camera lock、Part/MaterialZone selection/isolate/explosion、candidate undo/redo、Viewer a11y 和真实 Codex/human evidence。

Viewer 只有一个 WebGL context，继续只读 Runtime projection。永久 geometry/material/restore/export 仍回到 Codex 的 prepare/approval/confirm。

真实链路固定为：

```text
reference_import → geometry_prepare → artifact_readback_get
→ appearance_prepare → reference_compare_prepare → render_pass_get
→ visual_review_submit → 最多五轮 change_prepare
→ human_visual_review_submit → quality_get
→ candidate_confirm → version_diff
→ restore_prepare/restore_confirm
→ export_prepare/export_confirm
```

输出必须有 self-contained GLB、固定视图、diff、QualityReport、human receipt、immutable version、restore/export receipts，以及 Viewer/export/restart 同 hash 证据。

## 4. 质量门

### 4.1 几何/UV/PBR 硬门

- invalid index、non-finite、超阈值 degenerate triangle 为 0；
- 声明 solid 的 Part boundary/non-manifold edge 为 0；
- triangle 100% 绑定 `part_id + source_node_id + material_zone_id`；
- 同机器重复五次 program/mesh/GLB/report hash 一致；
- MaterialZone binding 完整，无 unused/unknown；
- UV finite、零面积 UV triangle 为 0，padding/density 满足 Recipe；
- tangent orthogonality/handedness/normal convention 通过；
- GLB 无 external URI，Runtime readback 与 Khronos Validator 0 error；
- restart/restore/export 后 pack/material/texture/GLB hash 不变。

### 4.2 当前可见视图门

- silhouette IoU ≥0.72；
- boundary F1（4 px）≥0.75；
- bbox 边缘平均误差 ≤5%，centroid 误差 ≤4%；
- 可见 landmark coverage ≥80%，weighted NME ≤8%；
- region median IoU ≥0.50，critical region 不低于 0.30；
- 用户对 likeness、geometry detail、material fidelity、editability 各评分 ≥4/5。

指标必须记录 reference/camera/mask/render/toolchain hash、阈值、实测值和 limitation。通过这些门只允许 `PARTIAL_VISIBLE_VIEW_PASS`。

## 5. MCP011–013 保留边界

- MCP011：持久 Job checkpoint、复杂并发/cancel race、kill-9、GC/reachability、全局配额和性能；
- MCP012：通用第三方 Skill/AssetPack publisher、安装/禁用/升级/回滚、签名和撤销；
- MCP013：Developer ID、hardened runtime、notarization、clean install、正式 Codex 配置、packaged Desktop/CLI E2E、filesystem/package export、升级失败回滚和跨类别真人质量。

MCP010 的单操作预算、first-party 固定 AssetPack、ad-hoc 开发 App 和当前机器人用户评分不得替代上述任务。

## 6. 每任务验证与证据

每个子任务先记录 dirty baseline，再按 `Schema/negative → Core/Worker → Runtime/MCP/Viewer → focused → aggregate → real Codex/visual/human` 顺序运行适用 Gate。共同命令：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
npm run contracts:check
git diff --check
```

Evidence 目录为 `docs/evidence/mcp010a/` 至 `mcp010f/`。每个 manifest 分别记录 PASS、FAIL、BLOCKED、NOT_RUN、命令 exit、contract/build/dependency/artifact hash 和脱敏检查；不得修改 MCP005–009 原始 receipt。

## 7. 禁止项

- 不内置模型、Provider、付费 API、远程 image-to-3D 或素材 API；
- 不安装 BlenderMCP、FreeCAD MCP、任意 Python/JavaScript/shader 插件或 GitHub Skill pack；
- 不让 `.blend`、Three.js scene、外部 validator、截图或自然语言成为产品真值；
- 不在 010A 提前增加当前 Schema/tool/Skill 数量；
- 不在缺少多视图时宣称 360，不在单个机器人通过后宣称通用高质量；
- 不 commit、merge 或 push，除非用户另行明确要求。
