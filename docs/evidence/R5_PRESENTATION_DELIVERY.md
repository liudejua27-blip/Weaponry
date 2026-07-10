# R5 Presentation Delivery Evidence

日期：2026-07-10

范围：证明 Weapon Concept 技术预览具备确定性轮廓抗锯齿、软接触阴影和可追溯 MP4 转台交付，并建立真实 Blender/Assimp 往返预检。它不证明照片级渲染、最终资产性能、真实 DCC 往返已在当前机器完成或制造 CAD。

## 实现

- 640×640 RGBA8 软件渲染在透明轮廓外增加固定 coverage 像素；
- 非 top 相机绘制确定性半透明软接触阴影；
- `antialias_mode` 与 `shadow_mode` 写入 package asset metadata 和 render JobEvent；
- `include_turntable_video` 默认 false，并要求 `include_render_png=true`；
- FFmpeg 使用 8 帧、8 fps、H.264、单线程、固定 CRF/preset 并移除动态 metadata；
- `Renders/turntable.mp4` 进入 render-set ZIP、主 ZIP 和 `ConceptExportManifest@1.files`；
- `ConceptExportRecord` 保存视频 SHA-256、byte size 和 MIME；
- `/turntable.mp4`、桌面 MP4 格式和 PNG 面板配套下载均读取同一不可变 Export；
- DCC runner 拒绝覆盖输入/提交中的 Module Pack，验证源 hash 不变、输出 GLB 2.0 可读及 vertex/triangle count 一致。

## 自动验证

```bash
npm run r5:presentation-gate
```

专项结果：

- `agent:r2-exports-smoke`：通过；MP4 SHA-256 `897bdf54f55a51b95813bff0294cc691921531781d0412d5f061c6a51e3134ad`，重复编码字节一致，ZIP/API/重启回读一致；
- `desktop:r3-concept-workbench-smoke`：通过；10 模块工作台下载 ZIP/GLB/OBJ/MTL/PNG/MP4，全程只创建 1 个 Export；
- 视觉工件：`output/playwright/r5-concept-preview.png`，已核对透明轮廓、模型可读性和模型下方软阴影；
- `assets:dcc-roundtrip-preflight`：返回 `blocked_dcc_not_configured`，Blender 与 Assimp 均为 null，因此当前没有真实 round-trip 证据。

## 未完成

- 正式 10–12 个 Blender 模块的渲染时间/内存阈值；
- 当前机器上的 Blender 或 Assimp 真实导入/再导出；
- 贴图、PBR 环境光、照片级材质、插帧和自定义视频参数；
- Blender/Cycles 或 Three.js GPU renderer 像素等价；
- 任何 B-Rep、STEP、3MF、结构强度、切片或制造 DFM 结论。
