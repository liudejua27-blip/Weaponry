# R5 Mesh / Assembly Quality Evidence

日期：2026-07-10

范围：证明服务端会读取 Version 绑定的不可变 ModuleGraph 与内容寻址 GLB，生成可持久化的首版几何/装配 Findings；不证明结构强度、制造可行性、使用安全、精确三角相交或完整 DFM。

## 实现

- 新增 `POST /api/v1/versions/{version_id}/quality-runs:inspect`；
- 固定规则集 `weapon-concept-geometry/1.0`；
- 解码 GLB 2.0 内嵌 Buffer 的 POSITION、NORMAL、TEXCOORD_0 与 indices accessor；
- 检查非法索引、三角数/清单不一致、退化三角形、法线、UV0、开放边、非流形边和 bounds；
- 以 `0.1 mm / 0.1°` 检查 Connector 世界 frame 对齐；
- 对未直连节点做世界 AABB 穿插筛查，超过较小组件体积 2% 时产生 warning；
- Finding 写入 `quality_findings`，报告写入 `quality_runs`，四步轨迹写入 Concept JobEvent@2；
- 桌面检查面板可以触发并显示状态与 Findings；
- 保留旧的客户端报告 ingestion 接口用于兼容，但实际检查按钮不依赖客户端自报。

## 自动验证

```bash
npm run r5:quality-gate
```

证据覆盖：

1. 导入实际 10 模块参考 Pack，并检查 9-node Graph 的 9 个 GLB；
2. 参考 Mesh 通过索引、退化面、法线、UV0、拓扑、triangle 与 bounds 检查；
3. 两组未直连 AABB 穿插被记录为可复核 warning；
4. 人为偏移前壳 5 mm 后报告变为 failed；
5. 合成负例命中退化面、开放边、非流形边与法线缺失；
6. 幂等 replay、冲突、四步 JobEvent、Finding 规范化表和 Agent 重启回读通过；
7. 浏览器从检查面板触发 API，并显示“通过”和非 CAD/DFM 边界声明。

## 未完成

- triangle/BVH 精确相交和异常间隙；
- 对称、隐藏几何、网格密度与 LOD；
- Finding 点击后的相机定位/局部高亮；
- HTML 报告、自动修复与规则配置 UI；
- 最终 Blender 资产 truth set、Tauri GPU/大资产性能；
- 任何 B-Rep、STEP、3MF、强度、切片或制造 DFM 结论。
