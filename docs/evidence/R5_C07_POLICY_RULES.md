# R5 C07 Policy Rules Evidence

日期：2026-07-10

范围：证明 `weapon-concept-geometry/1.3` 会读取 Version 绑定的 WeaponConceptSpec、ModuleGraph 与内容寻址 GLB，并对隐藏几何、网格密度/预算、当前 P0 LOD0 合同和模块占位对称目标产生确定性 Finding。它不证明结构强度、制造可行性、装配公差、使用安全、多 LOD 运行时或完整正式资产发布质量。

## 规则语义

- `mesh.duplicate_triangles`：按 `0.001 mm` 焊接顶点后，三个顶点集合相同的后续 triangle；
- `mesh.enclosed_components`：两个断开且各自封闭的 triangle component 无表面相交，同时一个 bounds 严格位于另一个 bounds 内，并由 closed-mesh containment 确认；
- `mesh.density_outlier`：实际表面积上的 `triangle / 1000 mm²`；装配至少三个有效模块时，超过中位数 8 倍提示复核；
- `mesh.triangle_budget`：所有 Graph 节点实际 triangle 总和不得超过 Version Spec 的 `max_triangle_count`；
- `mesh.lod0_contract`：当前只接受 canonical `MESH_/GEO_<module_id>_LOD0[_NN]`；LOD1/LOD2 继续拒绝，直到查看器切换、导出合同与质量门同时实现；
- `assembly.symmetry_deviation`：以 root 局部 Z 中面为基准，跨中面的模块 AABB 自配对；离开中面的模块只与同 category、尺寸和镜像中心在容差内的模块配对。`symmetric` 允许 5%，`mostly_symmetric` 允许 35%，`asymmetric` 跳过。

密度、对称和 bounds 间隙都是概念资产代理指标；文档与 UI 不把它们描述为工程分析。

## 自动门

```bash
npm run r5:c07-policy-gate
```

该门聚合合同/OpenAPI 生成一致性、Agent 静态检查、数据库 fresh/repeat migration、10 模块参考 Pack、质量 API/持久化/重启恢复、桌面类型检查/生产构建和浏览器 E2E。

合成 truth set 证明：

1. `100 mm` 外部封闭 cube 内的 `20 mm` 断开封闭 cube 命中 enclosed component；
2. 两个完全相同的封闭 box 命中 duplicate triangles；
3. 把 canonical GLB 的 Mesh/Node 改名为 LOD1 后命中 `mesh.lod0_contract`；
4. 三个 `100 mm` box 与一个 `1 mm` box 组成的装配只把小 box 标记为密度离群；
5. 84 个 box、共 1008 triangles 的模块超过 Spec 1000 预算；
6. root 中面模块加一个单侧模块，在 `symmetric` 目标下命中 symmetry deviation。

版本级 API 另以完整 9-node Arctic Patrol S1 参考 Graph 验证：原始 `mostly_symmetric` 不增加新 Finding；只把 Version Spec 改为 `symmetric` 后稳定报告 `22.222222% unmatched_modules=2`，报告、Finding、JobEvent 与 Agent 重启回读通过。请求已被取代的 `1.2` 返回 422，避免新旧规则语义混写。

## 仍需发布证据

- 将合成 truth set 迁移到正式 10–12 个 Blender 模块，统计误报、漏报和作者修复结果；
- 在 Tauri 对正式大网格测量检查时间、峰值内存与视口 overlay 成本；
- 实现 LOD1/LOD2 资产组、运行时切换和导出合同后，再把当前 LOD0 门扩展为真正的多 LOD 一致性检查；
- HTML 报告、规则配置 UI 与只针对安全几何编辑的自动修复；
- B-Rep、STEP、3MF、强度、切片与制造 DFM 属于后续 Engineering Pack。
