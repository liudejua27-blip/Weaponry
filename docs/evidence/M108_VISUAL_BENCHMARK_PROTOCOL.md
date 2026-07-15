# FGC-M108 独立视觉基准操作协议

状态：当前可执行；评分结果尚未收集。

目的：验证四领域同源 GLB 在 ForgeCAD 工作台的实际 PBR 视口中，是否达到任务卡所要求的比例、材质可读性和表面细节中位数门槛。它不是 Provider 评测、工程验证、制造评价或“照片级真实”声明。

## 1. 生成不可变审阅包

在仓库根目录执行：

```bash
npm run agent:m108-visual-benchmark-kit
```

该命令只从 `smoke_m108_visual_pbr.py` 的确定性 showcase 编译路径选择四份 GLB（每个已启用领域一份），写入被忽略的 `output/m108-visual-benchmark/`。`manifest.json` 记录每份 GLB 的 SHA-256、字节数、triangle、zone、实际 authored 材质 ID、规范 texture material、纹理集、纹理尺寸、受限 edge-finish primitive 数和固定工作室环境；`review-responses.json` 必须初始为空。当前审阅包要求五通道纹理均为材质专属、确定性生成的 128×128 PNG，且其 texture-set ID 以 `_builtin_v2` 结尾、map ID 含 `_v2_`、`version=2`，并至少回读一个受限 `bevel_approximation`；旧 v1 仅用于历史资产 readback，不能混入新审阅包，任何实际使用材质的 v1/v2 混版都会被源码 Gate 拒绝。这仍只是概念视觉资产，不是照片级或工程材质证明。

执行前或 CI 中可运行：

```bash
npm run agent:m108-visual-benchmark-kit-smoke
```

它只验证审阅包可复现、四领域齐全、每份 GLB 具有多 zone、至少五条实际材质/纹理绑定、统一 128×128 五通道纹理和受限 bevel readback，不产生评分。M108 PBR smoke 另按 primitive 的真实 material index/role 核对轮胎/握把、座舱/玻璃、灯带、关节/旋翼等受限绑定，并且只接受实际使用材质触发的 clearcoat 或 transmission/IOR；仅在 GLB 声明未使用扩展不算通过。汽车代表 fixture 还必须实际使用独立 index 7 的 `mat_automotive_paint`，其 coated texture set 与 clearcoat 不得复用 aluminum；删除或篡改已使用 clearcoat 必须拒绝。PBR smoke 解码八种内置材质的全部五通道，拒绝 8/12/16/18/28/32 px 格线；只对 metallicRoughness/normal 要求微变化，不把纯色 baseColor/AO/emissive 误判为逃避。authored material→规范 texture material、texture-set/map identity、完整元数据、PNG 字节、UV0 TextureInfo 和固定采样状态必须与 current/legacy 清单一致；更新自报 SHA、自定义 sampler/texture transform、未知材质和布尔伪索引都不能伪装为可信。当前 fixture 的内置视觉 primitive 还必须回读 `forgecad_visual_uv_repeat_mm=320`；G826 同时锁定封闭 primitive 外向绕序、非退化三角形和正有向体积，并拒绝错误重复元数据或超界 UV。G818 从最终 GLB POSITION accessor 要求固定连接外罩 AABB 与每个目标正体积交叠，并有体积位于目标 AABB 并集外；这些是固定 fixture 的纹理可见性/稳定网格 Gate，不是曲面实体相交或工程几何证明。

## 2. 开发视觉审计截图（不是评分）

生成审阅包后，可以运行：

```bash
npm run agent:m108-visual-benchmark-workbench-capture
```

CI 和本机可重复 renderer Gate 使用：

```bash
npm run desktop:m108-workbench-renderer-smoke
```

后一命令会在临时目录重新生成同源 kit，再启动真实工作台完成四领域捕获；设置 `FORGECAD_M108_RENDERER_OUTPUT_DIR=output/m108-workbench-renderer` 时才保留无评分截图工件。它不读取旧截图作为通过依据。

该 Gate 复用 R3 的真实 ForgeCAD 工作台和唯一 renderer/canvas，依次导入四领域 fixture，固定 `iso`、`cad_neutral` 与 `env_forgecad_room_studio_v1`，并使用环境合同中的前向 iso 方向与 `ShadowMaterial` 地面。GLB 的 metre→millimetre 换算先保留，再乘以确定性的视口 fit scale；当前展示对角线固定为 520 mm，不能由隐藏的 legacy graph 尺寸决定。kit manifest 的编译 `bounds_mm` 必须与 GLTFLoader 加载后的三轴毫米 bounds 一致；工作台再按实际 FOV、viewport aspect、OrbitControls 相机基和 8 个 bounds 角点求安全距离，初始视口及 1180×1024 resize 后捕获的 NDC 都必须位于 `[-0.9, 0.9]`。studio fog 移到完整对象之后，阴影接收面和 shadow camera 随当前 framed bounds 收敛；这些显示事实均不写回资产或 Snapshot。

`M108WorkbenchCapture@1` 除源 manifest、GLB/screenshot、load/render/preview/xray 与单 context 事实外，还必须把工作台实时应用的完整环境配方 canonicalize 后重新计算 SHA-256，并与 GLB 环境 hash 相同；baseColor/emissive 必须为 sRGB，metallicRoughness/normal/AO 必须为数据色彩空间。每个 fixture 还要记录 source bounds、初始与 resize 后 NDC、安全 fog 和正相机距离，并通过真实 renderer 上限：geometries ≤72、textures ≤48、draw calls ≤96、triangles ≤7,000、实际使用的嵌入 PBR texture ≤35、按 RGBA8 完整 mip chain 保守估算的纹理显存 ≤4 MiB。triangle 上限只因固定 24 段 cylinder/capsule 的 renderer pass 保守上界达到 6,776 而调整；加入四领域连接外罩后的最新捕获最大为 6,176 triangles、87 draw calls，其余预算未放宽。顺序载入四个 GLB 后仍使用同一 renderer/canvas，因此未释放资源会在后续 fixture 中累计并触发预算失败。四个 fixture 后还会让浏览器上传损坏 GLB，同时只向服务端控制路径转交合法 fixture；客户端解析必须明确失败，恢复 300–820 fog、ModuleGraph/空工作台、相机/地面/shadow camera，清空 bounds/NDC/PBR facts，并保留同一个 renderer/context。最新真实捕获已验证四个领域均为 `ready/glb_pbr`、`preview_mode=committed`、`xray=disabled`、环境 recipe hash 匹配、颜色空间有效、520 mm 展示对角线、readback bounds、初始/窄视口安全取景、失败恢复、预算通过和单 WebGL context；此处 `committed` 只是非 ghost 的视口状态，不是 Git 提交或新资产版本。

这条命令只用于开发视觉审计。其工件固定为 `purpose=development_visual_audit_only`、`score_status=not_scored`、`human_benchmark_evidence=false`；自动截图不会向 `review-responses.json` 写入任何内容，不能成为 reviewer，不能证明比例/材质/细节达到 4/5，也不能完成 M108。若截图暴露比例断裂、材质区不可读或细节重复，应继续修复或在人工评分中如实失败，不能选择性隐藏截图或降低门槛。

## 3. 独立评审步骤

1. 至少邀请 3 位未实现本任务的评审者；由组织者在流程外人工核验其身份及与 M108 实现工作的独立性，只在工件中记录不重复的匿名 ID 和独立性声明，不记录个人敏感信息。校验器不能从匿名 ID 自动证明真实身份或独立性。
2. 每位评审者在同一版本的 ForgeCAD 工作台逐一导入四份 `fixtures/*.glb`，使用 `cad_neutral`、等轴相机、默认工作室环境，并在评分前人工确认视口不是 ghost preview 且 xray 关闭。不得用软件概念 PNG、外部 glTF 查看器截图或参数化 ShapeProgram 回退代替。
3. 对每份资产确认视口属性 `data-blockout-load-state=ready`、`data-blockout-render-source=glb_pbr`，且 `data-blockout-embedded-pbr-material-count` 大于 0。这里“导入”只负责把 fixture 带入同一工作台；普通外部 GLB 即使能以 `external_reference` 正常只读显示，也不能评分。只有内容通过完整 PBR map 检查并报告 `glb_pbr` 才是有效基准。任一失败使本次 run 无效，不可用“看起来接近”补分。
4. 对每份资产独立给出 1–5 分：`proportion`（主次体比例与轮廓）、`material_readability`（多材质/PBR 区域的区分）和 `surface_detail`（接缝、边缘、重复视觉细节是否服从部件边界）。可附简短失败原因和截图 SHA，但不应记录 API Key、外部 URL、原始 Provider 内容或工程材料性能推断。
5. 将每位评审的声明和 4×3 分数写入 `review-responses.json`；每条 fixture review 必须记录 `pbr_load_failure: false`，以及 `viewport.load_state: "ready"`、`viewport.render_source: "glb_pbr"` 与大于零的 `embedded_pbr_material_count`。提交前核对 `kit_manifest_sha256` 与本次 `manifest.json` 一致。

评分 JSON 的最小结构如下；`reviewer_id` 可以是非敏感的匿名代号，但不能重复：

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
        "embedded_pbr_material_count": 3
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

在提交任何通过结论前运行：

```bash
PYTHONPATH=apps/agent:scripts .venv/bin/python scripts/validate_m108_visual_benchmark_scores.py \
  --kit output/m108-visual-benchmark \
  --responses output/m108-visual-benchmark/review-responses.json
```

该校验器不生成、补齐或修改评分；它要求至少三个不同 reviewer ID、每人提交独立性声明并覆盖四领域、同源 PBR 视口事实和 1–5 整数分数，再分别计算每个领域的三个维度中位数。ID 与声明可机器校验，真实身份和独立性只能由上述人工流程核验。`npm run agent:m108-visual-benchmark-score-validator-smoke` 仅使用临时合成合同 fixture 验证拒绝边界，绝不是人工评分证据。

## 4. 通过口径与禁止项

有效 run 需要人工确认 3 位或以上评审者确实与实现工作独立，并且每个领域 fixture 的 `proportion`、`material_readability`、`surface_detail` 所有有效评审者分数中位数都至少为 4/5；跨领域总中位数只作摘要，不能掩盖任何一个领域失败。四个 fixture 的领域 ID 必须对应，GLB 路径和内容哈希必须互不重复。任意同源 PBR GLB 加载失败、身份/独立性未人工核验、少于四领域、少于三个不同 reviewer ID、manifest 或 fixture GLB hash/readback 不匹配、分数不是 1–5 整数或参数外观回退冒充嵌入纹理，均为未通过，而非缺失值补齐。

通过后才可在 `CODEX_TASK_INDEX`、`DOCUMENTATION_STATUS` 和能力—Gate 矩阵把 M108 的人工基准项更新为有证据；在此之前，M108 仍为 `in_progress`，C105 继续 blocked。
