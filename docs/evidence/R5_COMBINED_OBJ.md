# R5 Combined OBJ / MTL Evidence

日期：2026-07-10

范围：证明静态 Weapon Concept combined GLB 可以确定性转换为可追溯 OBJ/MTL；不证明贴图迁移、完整 PBR 等价、Blender/Assimp round-trip、PNG/爆炸图、最终美术或制造 CAD。

## 实现

- `CreateConceptExportRequest.include_combined_obj` 显式控制 OBJ/MTL，旧调用默认 false；
- OBJ 转换只读取本次导出生成的 combined GLB，因此 GLB 与 OBJ 不会使用两套 ModuleGraph；
- 递归扁平化 active scene 的 node hierarchy；
- 烘焙 matrix/TRS、非均匀 scale、rotation、translation 与 mirror；
- 法线使用逆转置矩阵，负行列式时翻转 triangle winding；
- 输出稳定 `NODE_{node_id}__{module_id}` object/group 路径和 `v/vt/vn/f`；
- OBJ 明确声明 meter，匹配 glTF；
- glTF PBR factor 确定性投影为 MTL 的 `Kd/d/Ns/Ke`；
- `Model/combined.obj` 与 `Model/combined.mtl` 进入 Manifest 和不可变 ZIP；
- `ConceptExportRecord` 保存 OBJ hash/size，新导出返回值非空，旧导出仍可回读；
- `/combined.obj` 与 `/combined.mtl` 从同一个不可变 ZIP 读取；
- 桌面 OBJ 选项和配套 MTL 下载已启用。

## 自动验证

```bash
npm run r5:obj-gate
```

证据覆盖：

1. OBJ/MTL 文件、MIME、Manifest hash/size 与 ZIP 内容一致；
2. 两个 fixture module 产生 6 vertices 与 2 faces；
3. 稳定 wrapper/object 名保留 node/module provenance；
4. `86 mm / 33 mm` 转换为 `0.086 m / 0.033 m`，X mirror 后首顶点为 `[0.136, 0.008, 0]`；
5. 镜像 triangle winding 从 `1/1 2/2 3/3` 翻为 `1/1 3/3 2/2`；
6. 相同 combined GLB 重复转换得到完全相同 OBJ/MTL；
7. 非 TRIANGLES primitive 被显式拒绝；
8. 幂等 replay、直接 OBJ/MTL 下载和 Agent 重启回读通过；
9. 浏览器从真实参考 Pack 下载 OBJ 与 MTL，并检查 `v/vt/vn/f`、material 和稳定资产名。

## 未完成

- texture/image 文件与 MTL map 路径；
- metallic-roughness 到传统 MTL 的完整等价（该格式本身不具备等价表达）；
- sparse accessor、morph target、skin、animation 和非 triangle primitive；
- Blender、Assimp、Unity 或其他 DCC/引擎 round-trip；
- glTF Transform/Meshopt 优化、PNG、爆炸图和 turntable；
- 任何 B-Rep、STEP、3MF、强度、切片或制造 DFM 结论。
