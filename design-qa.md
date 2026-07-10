# ForgeCAD Weapon Concept 工作台设计 QA

- 日期：2026-07-10
- 参考图：`/Users/liuchongjiang/Desktop/e9d4239c-ee36-44de-9161-5020d2fcb329.png`
- 实现入口：`http://127.0.0.1:1420/#/cad`
- 实现文件：`apps/desktop/src/features/cad-workbench/CadWorkbenchPanel.tsx`
- 目标视口：1536 × 1024
- 默认状态：概念阶段、选择工具、等轴视图、网格开启、`v5`、核心外壳、GLB

## 设计结论

参考图的九区高密度桌面工作台结构继续保留，产品语义已按 Weapon Concept Pack 重构：

| 区域 | 当前定义 |
| --- | --- |
| 顶部阶段 | 概念、组装、精修、检查、展示 |
| 左栏 | 项目/版本、AI 设计助手、概念比例输入 |
| 中央 | 可交互 Three.js 概念模型视口 |
| 底部抽屉 | 组件、方案、版本、时间线 |
| 右侧检查器 | 参数、外观、连接、检查 |
| 导出 | GLB、OBJ、PNG、REPORT 概念交付包 |
| 状态栏 | 当前阶段、选择、ModuleGraph 预览、单位与网格 |

原型不再默认展示 STEP、3MF、DFM 或“制造导出”，避免把尚未实现的 Engineering Pack 混入 P0。

## 核心交互

- 五阶段导航可切换并同步状态栏；
- 视口选择、移动、旋转、测量、截面、网格与线框控制可操作；
- 组件分类、搜索和选择可操作；
- 底部“组件/方案/版本/时间线”可切换；
- 右侧“参数/外观/连接/检查”可切换；
- 参数修改会更新程序化预览；
- AI 指令生成 ChangeSet 风格提示；
- 展示导出格式可选择。

## 资产与真实性

当前中心模型是用于验证布局和交互的 Three.js 程序化概念原型。它不是正式 GLB 模块包，也没有真实连接器、ModuleGraph、质量报告或导出任务；这些能力按 R2–R5 实现后才能把原型状态改为产品状态。

## 视觉验收

- 使用参考图的深海军蓝表面、低对比边框、蓝色主状态和紧凑桌面密度；
- 图标统一来自 Phosphor，没有 emoji 或手工 SVG；
- 1536 × 1024 是主验收视口，移动端不在产品范围；
- 先前基于参考图完成了模型占比、相机、光照、金属层次、红色特征和滚动条的三轮对照修正；本次信息架构修改需重新跑一次同视口截图对照。

## 验证命令

```bash
npm run desktop:typecheck
npm run desktop:build
npm run desktop:p0-context-continuity-smoke
```

自动化状态：`desktop:typecheck`、`desktop:build`、`desktop:p0-context-continuity-smoke` 与 `r1:gate` 已通过。应用内浏览器本次未能附着到新标签页，因此 1536 × 1024 的新信息架构截图对照仍待复核，不将自动化构建通过冒充视觉验收通过。
