# FGC-M108B 独立视觉基准操作协议

状态：协议已冻结；当前只能生成 `production_concept` 开发预检包。M108A、K003 与 C105 已通过；M108B 的 12 份 Recipe-backed 正式审阅资产和独立真人评分尚未完成。

历史兼容：文件名、`M108VisualBenchmark*` Schema 和 `agent:m108-*` 命令保留旧 M108 名称。按 ADR-0015，自动生产工件 Gate 归 `FGC-M108A`，本协议中的 Recipe 资产与真人 `4/5` 退出门归 `FGC-M108B`。

目的：验证四领域 Recipe-backed production GLB 在同一 ForgeCAD PBR 工作台中的比例、材质可读性和表面细节是否达到生产级概念资产基线。它不是 Provider 评测、照片级保证、工程验证、制造评价或对所有提示的普遍质量承诺。

## 1. M108A 工件前置条件

进入人工审阅前，每份 GLB 必须先通过 M108A 的生产工件合同：

- 同一不可变 ShapeProgram/AgentAssetVersion 可分别派生 `interactive_preview` 与 `production_concept`，不创建第二版本链；
- 正式审阅只接受 `artifact_profile_id=production_concept`；
- GLB root extras 与 `GeometryCompileReadback@2` 携带完全匹配的 profile manifest/hash；
- production 使用 48 段旋转体、10 段 capsule 半球、Loft/Sweep 平滑法线、512×512 v4 五通道 PBR；
- 实际使用的 texture-set ID 以 `_builtin_v4` 结尾，map ID 含 `_v4_`，五通道为 base color、metallic-roughness、normal、occlusion、emissive；
- Part、Material Zone、material、operation/output role 和 source-operation 身份与同源 preview 保持一致；
- GLB、readback、质量、正式展示、下载和导出使用同一 production 工件；profile 缺失、篡改、128/512 或 v3/v4 混用均拒绝；
- production 派生缓存使用内容寻址对象，不把 GLB/base64 写入 SQLite、事件、日志或索引。

这些条件由以下自动命令验证：

```bash
npm run agent:m108-production-concept-smoke
npm run agent:m108-gate
npm run desktop:m108-workbench-renderer-smoke
```

自动 Gate 通过只能写“生产概念工件管线已验证”，不能写“视觉已达到生产级概念资产基线”。

## 2. 当前开发预检包

在仓库根目录执行：

```bash
npm run agent:m108-visual-benchmark-kit
```

当前命令从固定 showcase 路径生成四份 `production_concept` GLB，写入被忽略的 `output/m108-visual-benchmark/`。`manifest.json` 记录 GLB SHA-256、字节数、artifact profile、triangle、bounds、Material Zone、实际材质/纹理、512×512 v4 五通道和工作室环境；`review-responses.json` 初始必须为空。

可重复性检查：

```bash
npm run agent:m108-visual-benchmark-kit-smoke
```

现有包适合验证 M108A production profile、工作台加载、PBR readback、相机和资源预算，但固定资产仍主要由绝对坐标 primitive、Loft 和 Sweep 组成。它没有证明 `EditableComponentRecipe@1` 的 child slot、connector/pivot、局部变换、语义比例和可复用完整部件，因此只能作为 `M108B preflight`，不能直接提交为 M108B 正式评分证据。

## 3. M108B 正式审阅包

C105 完成后，正式包必须满足：

1. 四领域每个领域至少 3 份 `EditableComponentRecipe@1` 实例化的 production fixture；
2. 每份 fixture 记录 Recipe ID/version/hash、实例化 provenance、关键 role、Profile/Section/feature template、child slot、connector/pivot、语义比例、Material Zone 和 production texture provenance；
3. 每份 fixture 通过 M108A、Q003 与 G826 自动硬门；
4. 每领域在评分前冻结代表 fixture，不由 V003 或评分结果事后挑选最高分；
5. `manifest.json` 明确标记 Recipe-backed 事实；缺少该事实时评分校验必须拒绝；
6. 固定资产只表达非功能外观。未来武器领域仅限虚构游戏、影视和展示道具，不包含现实武器机构、制造尺寸、材料配方、加工步骤或性能建议。

M108B-05 已交付的只是正式合同与校验器骨架，正式路径与旧预检路径完全分离：`agent:m108b-formal-benchmark-kit` 只消费未来由 M108B-04 生成并在评分前冻结的 `M108BFormalFixtureSourceManifest@1`，写出 `M108BFormalVisualBenchmarkKit@1`。它要求恰好 12 份不同 hash 的真实 GLB、每领域 3 份，并冻结 Recipe/ref/hash、registry hash/lock、candidate/provenance、role/Profile/Section/feature、child slot、connector/pivot、semantic binding、Material Zone、v4 PBR map hash。每份还必须带三个不同的 `M108BGateEvidence@1` M108A/Q003/G826 `passed` report（各自 Gate ID、execution ID、文件/hash、source-GLB 与同一真实 readback hash 绑定）；不能将一份 readback JSON 复制三次冒充执行证据。还必须带 `M108BRendererCaptureEvidence@1` 的真实 renderer capture 文件/hash/source-GLB/逐项预算，并锁定 ForgeCAD 工作台 renderer、固定工作室环境 hash、iso 相机、ready/glb_pbr、实际嵌入 PBR 材质和单一 WebGL context；预算不可只由 source manifest 自报。缺 source、GLB、Gate report、capture 或任何漂移均明确 blocked；它不会生成资产、选择样本或接受 Python showcase shortcut。当前仓库尚不存在可消费的 12-fixture formal source/capture，因此不产生正式 kit 或评分证据。

正式评分只能运行：

```bash
npm run agent:m108b-formal-score-validator -- \
  --kit output/m108b-formal-visual-benchmark \
  --responses output/m108b-formal-visual-benchmark/review-responses.json
```

该校验器只接受正式 kit，拒绝旧四 fixture showcase、少于 12、领域分布错误、评分后选择、source/GLB/hash/readback/PBR 漂移，以及任何非真人、非独立或自动/代理评分。每位真人必须完整评完 12 份；每个 fixture 三项中位数和每领域三项聚合中位数都必须至少为 `4/5`。

## 4. 开发工作台捕获

生成预检包后可运行：

```bash
npm run agent:m108-visual-benchmark-workbench-capture
```

CI/本机 renderer Gate：

```bash
npm run desktop:m108-workbench-renderer-smoke
```

该 Gate 复用真实 ForgeCAD 工作台和唯一 renderer/canvas，依次导入四领域 production GLB，固定 `iso`、`cad_neutral` 与 `env_forgecad_room_studio_v1`。它校验 metre→millimetre、520 mm 展示对角线、真实 bounds、初始及 1180×1024 resize 的安全取景、环境 hash、PBR 色彩空间、调试辅助隐藏、损坏 GLB 恢复和单 WebGL context。

当前 production renderer 上限为：

- geometries ≤ 72；
- textures ≤ 48；
- draw calls ≤ 96；
- triangles ≤ 24,000；
- 实际嵌入 PBR texture ≤ 35；
- RGBA8 完整 mip chain 估算纹理显存 ≤ 64 MiB。

当前 M108A 检查点四领域 production 捕获为 7,308/68、9,148/78、8,116/96、13,704/53（triangles/draw calls），GLB 约 2.1–2.7 MB，估算 GPU 约 35–49 MiB。preview 的 T003 预算保持独立，不得为了 production 全局放宽交互档。

捕获工件固定为：

```text
purpose=development_visual_audit_only
score_status=not_scored
human_benchmark_evidence=false
```

自动截图不能成为 reviewer、不能写入 `review-responses.json`、不能证明三项达到 `4/5`，也不能完成 M108B。

## 5. 独立评审步骤

只有第 3 节 Recipe-backed 正式包可执行本节：

1. 至少邀请 3 位未参与 M108B/C105 资产与实现工作的真人评审者。组织者在流程外核验身份和独立性；工件只记录不重复匿名 ID 和独立性声明，不记录个人敏感信息。
2. 每位评审者在同一版本 ForgeCAD 工作台逐一查看冻结的 `production_concept` GLB，使用固定工作室环境、等轴相机、非 ghost preview、xray 关闭状态。不得用软件概念 PNG、外部查看器或参数外观回退代替。
3. 视口必须报告 `load_state=ready`、`render_source=glb_pbr`、`embedded_pbr_material_count>0`，且 GLB/profile/Recipe/hash 与 manifest 一致。任一失败使本次 run 无效。
4. 对每个领域 fixture 分别给出 1–5 整数分：
   - `proportion`：主次体比例、完整轮廓和部件关系；
   - `material_readability`：多材质/PBR 区域与透明、橡胶、涂层等外观区分；
   - `surface_detail`：接缝、边缘、流线、图案和重复细节是否服从部件边界。
5. 可记录简短失败原因和截图 SHA，不记录 API Key、外部 URL、Provider 原文或工程材料/性能推断。
6. 将每位评审的声明和完整评分写入 `review-responses.json`，并核对 `kit_manifest_sha256`。

评分 JSON 保留历史兼容 Schema：

```json
{
  "schema_version": "M108VisualBenchmarkResponses@1",
  "kit_manifest_sha256": "<manifest sha256>",
  "responses": [{
    "reviewer_id": "reviewer_01",
    "independent_of_implementation": true,
    "fixture_reviews": [{
      "fixture_id": "<fixture id>",
      "pbr_load_failure": false,
      "viewport": {
        "load_state": "ready",
        "render_source": "glb_pbr",
        "embedded_pbr_material_count": 5
      },
      "scores": {
        "proportion": 4,
        "material_readability": 4,
        "surface_detail": 4
      }
    }]
  }]
}
```

正式提交前运行：

```bash
PYTHONPATH=apps/agent:scripts .venv/bin/python scripts/validate_m108_visual_benchmark_scores.py \
  --kit output/m108-visual-benchmark \
  --responses output/m108-visual-benchmark/review-responses.json
```

校验器不生成、补齐或修改评分。M108B-05 的正式校验器已对 Recipe-backed manifest/provenance、production GLB/readback、v4 PBR map hash、独立 Gate report、renderer capture、12-fixture 冻结与独立真人完整评分执行拒绝边界；它只能在后续存在冻结 source/capture 时验证提交，不能把本阶段的自测或预检包变为真人退出证据。

## 6. 通过口径

有效 run 必须满足：

- 至少 3 位真实、与实现独立的评审者；
- 四领域 Recipe-backed fixture 全部通过 production GLB/readback；
- 每个领域的 `proportion`、`material_readability`、`surface_detail` 三项中位数分别不少于 `4/5`；
- 任何领域失败都使 M108B 未通过，跨领域总分不能掩盖失败；
- 不得降低门槛、选择性隐藏截图、伪造 reviewer，或用 Codex/其他代理评分补齐。

只有通过后，才可在任务索引、状态账本、能力矩阵和用户指南中把基准覆盖的四领域输出写为“生产级概念资产基线”。仍不得写成照片级保证、工程 CAD、制造级模型或对任意提示的普遍保证。
