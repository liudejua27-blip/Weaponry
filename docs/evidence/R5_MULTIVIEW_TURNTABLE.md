# R5 Multiview / Turntable Evidence

日期：2026-07-10

范围：证明同一 Weapon Concept Export 可以确定性生成三张正交图和 8 帧转台，并作为一个可追溯 render set 交付；不证明视频编码、照片级渲染、正式资产性能或制造 CAD。

## 实现

- front：相机从 +Z 看向原点，Y 向上；
- side：相机从 +X 看向原点，Y 向上；
- top：相机从 +Y 看向原点，屏幕上方为 -Z；
- turntable：相机保持正 Y 仰角，绕 Y 轴均匀采样 8 个方向；
- 所有帧复用一次 scene flatten 后的三角形和材质，不改变 ModuleGraph；
- 13 张 PNG：preview、exploded、3 views、8 turntable frames；
- `Renders/render-set.zip` 确定性收集全部 PNG；
- 主 ZIP 和 `ConceptExportManifest@1.files` 同时记录独立 PNG 与 render-set ZIP；
- `ConceptExportRecord` 保存 render-set hash/size、view count 和 frame count；
- 提供 `/views/{front|side|top}.png`、`/turntable/{0..7}.png`、`/renders.zip`；
- 未知 view 和越界 frame 返回结构化 `INVALID_REQUEST`；
- 桌面在 Version 相同且所需工件存在时复用最近 Export，避免格式间漂移。

## 自动验证

```bash
npm run r5:multiview-gate
```

证据覆盖：

1. front/side/top 均为 640×640 RGBA8，至少两个投影字节不同；
2. 实际 3D 参考资产的三个视图均可见且方向不同；
3. 8 帧 turntable 的 SHA-256 全部不同；
4. render-set ZIP 精确包含 13 张图，字节与主 ZIP 独立条目一致；
5. Manifest/API hash、size、MIME、view/frame count 一致；
6. 非法 view 和 frame 8 被拒绝；
7. 幂等 replay、直接下载和 Agent 重启回读通过；
8. 浏览器 ZIP/GLB/OBJ/PNG/多视图下载全过程只创建 1 个 Export；
9. 人工视觉核对 front、top 和 frame-000，确认不是重复等距图或空白帧。

视觉工件：

- `output/playwright/r5-concept-front.png`
- `output/playwright/r5-concept-top.png`
- `output/playwright/r5-concept-turntable-000.png`

## 未完成

- GIF/MP4/WebM turntable 视频编码；
- 帧间抗锯齿、阴影、环境光照和贴图；
- 自定义相机、分辨率、背景和帧数 UI；
- 最终 10–12 个 Blender 资产的时间/内存门；
- Blender/Cycles、Three.js GPU renderer 或其他 DCC 的像素等价；
- 任何 B-Rep、STEP、3MF、强度、切片或制造 DFM 结论。
