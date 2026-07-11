# R5 Combined GLB Evidence

日期：2026-07-10

范围：证明静态 Weapon Concept ModuleGraph 可以生成可追溯单一 GLB；OBJ/MTL 与 PNG 证据分别拆到 `R5_COMBINED_OBJ.md`、`R5_RENDER_PNG.md`，本页不证明纹理/动画/skin、Meshopt/Draco、最终美术或制造 CAD。

## 实现

- `CreateConceptExportRequest.include_combined_glb` 固定为 true；
- 源模块 GLB 仍原样进入 `Modules/`；
- 合并结果进入 `Model/combined.glb`；
- 合并 buffer、bufferView、accessor、mesh、material 和 node 索引；
- 完全相同的 material JSON 去重；
- Graph 毫米位置转换为 glTF 米；
- Euler XYZ 转 quaternion；
- `mirror_axis` 转换为 wrapper node 有符号 scale；
- wrapper 命名 `NODE_{node_id}__{module_id}`，extras 保留 node/module/mirror provenance；
- combined GLB hash/size 进入 `ConceptExportRecord` 和 `ConceptExportManifest.files`；
- ZIP 和 `/api/v1/exports/{export_id}/combined.glb` 读取同一不可变包；
- skin、animation、camera、texture/image/sampler、required extension 或扩展 primitive 当前显式拒绝。

## 自动验证

```bash
npm run r5:combined-glb-gate
```

证据覆盖：

1. GLB envelope、wrapper node 名称和 source module 数；
2. ZIP Manifest 文件 hash 与大小；
3. 独立 GLB 下载与 ZIP 内 GLB 字节一致；
4. 幂等 replay 与冲突；
5. Agent 重启后 ZIP/GLB 回读一致；
6. `86 mm / 33 mm` 转换为 `0.086 m / 0.033 m`；
7. X 镜像转换为 `scale[-1,1,1]`；
8. 桌面 GLB 按钮下载以参考 Pack 生成的多 mesh 文件，header 为 `glTF`；
9. 源 ZIP 继续保留 Spec、Graph、Quality、Manifest 和独立模块。
10. 2026-07-11 用工作台 E2E 导出的 10 模块 reference combined GLB 运行 Blender 4.2.22 往返；源 SHA-256 保持不变，840 顶点 / 420 三角保持一致。
11. 2026-07-11 用隔离 Agent 导出的十模块 visual candidate combined GLB 运行 Blender 4.2.22 往返；源 SHA-256 保持不变，输出为 25808 顶点 / 10716 三角。该资产仍待人工审批。

## 未完成

- glTF Transform prune/dedupe/meshopt 与纹理合并；
- textured GLB、skin、animation；
- 多视图、转台和照片级渲染；
- 正式 Blender 资产上的对称/隐藏几何/密度 truth set 与多 LOD 运行时；合成策略规则已见 `R5_C07_POLICY_RULES.md`；
- Unity/其他引擎的 combined GLB round-trip；
- 最终 Blender 资产的大小、draw call 和视觉验收。
