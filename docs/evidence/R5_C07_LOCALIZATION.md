# R5 C07 Finding Localization Evidence

日期：2026-07-10

范围：证明 `weapon-concept-geometry/1.2` 已为精确穿插 Finding 保存可回读的局部几何引用，在桌面同时高亮双方节点与相交三角形，并为已通过 Connector 对齐的组件增加保守表面间隙提示。它不是结构强度、制造可行性、装配公差或使用安全证明。

## 数据与规则

- `ModelQualityReport@1` 的 Finding 可带 `geometry_refs`；每个引用包含 `node_id`、最多 16 个 triangle index 和一一对应的毫米世界坐标；
- triangle BVH/SAT narrow phase 返回确切命中对，检查器按节点去重并截断为可控的 UI 证据，不再只保存命中数量；
- `quality_finding_geometry_refs` 以 `(finding_id, ref_index)` 规范化持久化引用，同时保留 report JSON 作为完整不可变报告；migration `0013` 可重复执行并已进入 foundation smoke；
- 对直接相连且 Connector frame 已对齐的节点，`assembly.connected_surface_gap` 计算两个世界 AABB 的分离距离，超过 `2 mm` 生成 warning；它是保守 bounds 距离，不是精确 mesh 最近点或制造公差；
- `1.1` 已被检查入口拒绝，避免把新的 provenance 与间隙语义写成旧规则版本。

## 工作台行为

- 点击含几何引用的 Finding 会选择首个有效节点，并以全部关联节点的联合包围盒重新框选相机；
- 双方模块使用红色 emissive 高亮，选择态仍保留独立的蓝色语义；
- 世界三角形使用不受深度遮挡的红橙线框叠加，视口暴露 `data-quality-node-ids` 与 `data-quality-triangle-count` 供 E2E 验证；
- 新质量运行或版本切换会清除旧高亮，Three.js scene 卸载时释放 overlay geometry/material。

## 自动门

```bash
npm run r5:c07-localization-gate
```

该门聚合合同生成一致性、Agent 静态检查、13 个 migration 的 fresh/repeat 路径、参考 Pack、质量 API/数据库/重启恢复、桌面类型检查/生产构建和浏览器 E2E。

质量 truth set 断言：

- 9 个参考模块全部通过 Mesh 基础检查；
- 参考 9-node Graph 稳定得到 2 个未连接组件穿插 warning；
- 每个穿插 Finding 含双方 geometry refs，triangle index 与世界坐标等长；
- 合成 Connector 对齐、但烘焙几何平移的节点命中一次间隙 warning，测得 `100.000005 mm`；
- 本次检查的 2 个穿插 Finding 规范化为 4 条 geometry ref 记录；
- `1.1` 返回 422，5 mm Connector 错位仍失败，幂等 replay、JobEvent 和 Agent 重启回读通过。

桌面 E2E 使用完整 9-node Graph 和含真实 geometry refs 的质量响应，断言 Finding 点击后双方 node id 完全匹配，局部 overlay 数量为 20，并生成截图：

`output/playwright/r5-quality-triangle-highlight.png`

## 剩余 C07

- 对称、隐藏几何、网格密度与 LOD 的规则和失败样本；
- 正式 10–12 个 Blender 资产的完整质量 truth set 与 Tauri GPU/大网格阈值；
- HTML 报告、可配置规则和仅对安全几何编辑开放的自动修复；
- B-Rep、STEP、3MF、强度、切片与制造 DFM 属于后续 Engineering Pack。
