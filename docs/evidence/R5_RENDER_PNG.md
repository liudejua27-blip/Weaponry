# R5 Preview / Exploded PNG Evidence

日期：2026-07-10

范围：证明 Weapon Concept combined GLB 可以在本地 Agent 内确定性生成透明技术预览和爆炸 PNG；front/side/top 与 turntable 证据已拆到 `R5_MULTIVIEW_TURNTABLE.md`，本页不证明照片级渲染、正式资产性能或制造 CAD。

## 实现

- `CreateConceptExportRequest.include_render_png` 显式控制渲染，旧调用默认 false；
- 渲染输入固定为同一 Export 的 combined GLB，不读取桌面 canvas；
- 通过已有 scene flatten 应用 Graph TRS、非均匀缩放、镜像和源 node hierarchy；
- 640×640 RGBA8、透明背景、三分之四正交投影、自动取景；
- CPU z-buffer、三角形光栅化、基础材质颜色、线性到 sRGB 和简单方向光；
- 共面深度使用固定 epsilon 与稳定导出顺序，避免随机 z-fighting；
- exploded GLB 只在内存副本中修改 wrapper translation，按装配中心径向分离；
- 重合中心使用 node ID SHA-256 派生稳定方向；
- `Renders/preview.png`、`Renders/exploded.png` 进入 Manifest 和不可变 ZIP；
- `ConceptExportRecord` 保存两张 PNG 的 hash/size，旧导出仍可回读；
- `/preview.png`、`/exploded.png` 从同一不可变 ZIP 读取；
- 桌面 PNG 选项和 exploded 配套下载已启用。

## 自动验证

```bash
npm run r5:render-gate
```

证据覆盖：

1. PNG signature、IHDR、640×640、RGBA8 与 filter 0；
2. 同时包含透明像素和非透明模型像素；
3. preview 与 exploded PNG 字节不同，exploded JobEvent 记录非零距离；
4. 相同 combined GLB 重复渲染字节完全一致；
5. ZIP/Manifest/API hash、大小、MIME 和直接下载一致；
6. 幂等 replay 与 Agent 重启回读通过；
7. 浏览器从真实参考 Pack 下载两张 PNG 并保存证据工件；
8. 人工视觉核对确认普通图取景完整，爆炸图分离 core/front/grip，颜色和轮廓可读。

视觉工件：

- `output/playwright/r5-concept-preview.png`
- `output/playwright/r5-concept-exploded.png`

## 未完成

- 抗锯齿、阴影、环境光照、贴图与照片级材质；
- turntable 视频；
- 最终 10–12 个 Blender 资产的时间/内存性能门；
- Blender/Cycles、Three.js GPU renderer 或其他 DCC 的像素等价；
- 任何 B-Rep、STEP、3MF、强度、切片或制造 DFM 结论。
