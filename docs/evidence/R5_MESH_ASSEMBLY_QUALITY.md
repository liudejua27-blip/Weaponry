# R5 Mesh / Assembly Quality Evidence

日期：2026-07-10

范围：证明服务端会读取 Version 绑定的不可变 ModuleGraph 与内容寻址 GLB，生成可持久化的几何/装配 Findings；精确穿插算法的详细 truth set 见 `R5_EXACT_INTERSECTION.md`。不证明结构强度、制造可行性、使用安全或完整 DFM。

## 实现

- 新增 `POST /api/v1/versions/{version_id}/quality-runs:inspect`；
- 本页记录当时的 `weapon-concept-geometry/1.1`；当前 `1.2` 的定位与间隙证据见 `R5_C07_LOCALIZATION.md`；
- 解码 GLB 2.0 内嵌 Buffer 的 POSITION、NORMAL、TEXCOORD_0 与 indices accessor；
- 检查非法索引、三角数/清单不一致、退化三角形、法线、UV0、开放边、非流形边和 bounds；
- 以 `0.1 mm / 0.1°` 检查 Connector 世界 frame 对齐；
- 对未直连节点做世界 triangle BVH broad phase、SAT narrow phase与 closed-mesh containment；
- Finding 写入 `quality_findings`，报告写入 `quality_runs`，四步轨迹写入 Concept JobEvent@2；
- 桌面检查面板可以触发并显示状态与 Findings，点击 Finding 会选择并聚焦关联节点；
- 保留旧的客户端报告 ingestion 接口用于兼容，但实际检查按钮不依赖客户端自报。

## 自动验证

```bash
npm run r5:quality-gate
```

证据覆盖：

1. 导入实际 10 模块参考 Pack，并检查 9-node Graph 的 9 个 GLB；
2. 参考 Mesh 通过索引、退化面、法线、UV0、拓扑、triangle 与 bounds 检查；
3. 两组未直连真实 triangle 穿插被记录为可复核 warning；
4. 人为偏移前壳 5 mm 后报告变为 failed；
5. 合成负例命中退化面、开放边、非流形边与法线缺失；
6. 幂等 replay、冲突、四步 JobEvent、Finding 规范化表和 Agent 重启回读通过；
7. 浏览器从检查面板触发 API，显示“通过”和非 CAD/DFM 边界声明，并点击 Finding 聚焦 `node_core`。

## 当时未完成、现由 1.2 完成

- 已连接组件异常间隙与相交三角形局部高亮；
- 第二关联节点框选与局部相交区域高亮。

## 仍未完成

- 对称、隐藏几何、网格密度与 LOD；
- HTML 报告、自动修复与规则配置 UI；
- 最终 Blender 资产 truth set、Tauri GPU/大资产性能；
- 任何 B-Rep、STEP、3MF、强度、切片或制造 DFM 结论。
