# R5 C07 Exact Intersection Evidence

日期：2026-07-10

范围：证明当时的 `weapon-concept-geometry/1.1` 已把未连接组件的历史 AABB 提示升级为世界空间三角形相交与封闭网格包含检查，并把 Finding 连到桌面节点聚焦。这是历史证据；局部 provenance、双节点高亮与已连接组件间隙已由 `1.2` 和 `R5_C07_LOCALIZATION.md` 取代。它不证明结构强度、制造可行性、使用安全或完整 C07。

## 算法与边界

- 从 Version 绑定的不可变 GLB 解码 triangle，并在模块 TRS、非均匀 scale 与 mirror 后转换为毫米世界坐标；
- 每个模块构建确定性 BVH，叶节点最多 8 个 triangle；
- triangle AABB 通过后，用法向量、9 组 edge cross edge 与共面分离轴执行 SAT，接触按相交处理；
- 两个网格均无 boundary/non-manifold edge 且表面没有相交时，以三条固定非轴向射线多数奇偶判断完整包含；
- 单个模块对最多记录 128 个表面相交对，Finding 显式报告 `capped`；
- 只检查没有直接 ModuleGraph edge 的节点对。已连接模块允许 Connector 接口接触或嵌合，并继续由 `0.1 mm / 0.1°` 对齐门负责；
- Finding 点击选择首个有效 node id 并让 Three.js 相机重新框选该节点；当前不高亮具体 triangle。

## 自动门

```bash
npm run r5:c07-intersection-gate
```

该门覆盖：

1. 交叉 triangle 为 true；
2. AABB 有交集但实际分离的共面 triangle 为 false；
3. 边界接触为 true；
4. 不同平面的分离 triangle 为 false；
5. 无表面交叉的内外封闭 cube 命中 containment；
6. 128×128 共 16,384 个分离候选由 BVH 剪枝，窄相位测试数低于笛卡尔积的 1%；
7. 10 模块参考 Pack 的 9-node Graph 稳定得到 2 个 `assembly.unconnected_triangle_intersection` warning；
8. 人为 5 mm Connector 错位仍为 failed，证明精确穿插没有取代接口规则；
9. 幂等冲突、JobEvent、数据库规范化和 Agent 重启回读；
10. 浏览器点击 `geometry.ruleset` Finding 后，状态栏选择与视口 `data-focus-node-id` 都切换为 `node_core`。
11. 请求历史 `weapon-concept-geometry/1.0` 会返回 422，避免把新算法结果写成旧规则版本。

## 当时剩余、现由 1.2 完成

- 已连接组件异常间隙的保守 AABB 距离规则；
- 相交 triangle id、双方节点框选与局部高亮。

## 仍未完成

- 对称、隐藏几何、密度与 LOD truth set；
- 正式 Blender 资产和 Tauri 大网格性能阈值；
- HTML 报告、规则配置 UI 和安全自动修复。
