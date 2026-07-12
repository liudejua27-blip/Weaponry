# ForgeCAD CAD 工作台设计 QA

- 日期：2026-07-12
- 参考视觉真相：`/Users/liuchongjiang/.codex/attachments/03c0589c-29dd-4070-8d2c-d30653758eff/image-1.png`
- 实现截图：`artifacts/design-qa/cad-workbench-candidate-v2-iso-bright.png`
- 同输入对照：`artifacts/design-qa/cad-workbench-reference-candidate-v2.png`
- 实现入口：本机 Tauri `武神 Forge.app`，`CadWorkbenchPanel`
- 截图视口：1238 × 768（参考按同高 768 px 对齐）；状态：概念、等轴视图、候选十模块 Pack、网格开启。

## Findings

- [P1] 中央资产的精密硬表面密度仍明显低于参考。
  - Location：中央 Three.js 视口 / 候选 Pack。
  - Evidence：参考中的主体、前端、握持、顶部附件有连续壳体、开孔、分层轨道和材质微差；候选已使用真实 GLB 的楔形、倒角、表面轨道、握持纹理和视觉管件，但仍以独立模块块体为主。
  - Impact：用户的首要感知仍是“模块样机”，而不是参考图级别的高精度未来概念资产。
  - Fix：继续在 `.blend` 源中人工细化主壳体/握持/顶部模块的连续曲面、开孔与面板节奏；保持相同 Module ID 和 Connector，再经 Pack、组合、质量和本机 Tauri 回归。

- [P1] 参考图的底部组件缩略图和右侧分析密度高于当前工作台。
  - Location：底部组件检视器、右侧检查面板。
  - Evidence：参考同时展示多张资产缩略图和 DFM 摘要；当前默认底部保持紧凑且候选模块尚未获得正式质量结论。
  - Impact：当前首屏在“精密 CAD 检视”上的信息密度偏低。
  - Fix：在不伪造质量/审阅标签的前提下，用真实缩略图、组件参数和实际质量报告填充候选；候选质量仅显示 warning/待审，不能显示正式通过。

- [P2] 当前本机截图小于参考窗口，左/右栏和文字相对更紧凑。
  - Location：整页布局与字体节奏。
  - Evidence：对照中当前 1238 px 窗口下的面板更窄，参考为 1536 px 设计稿。
  - Impact：中等；产品定位仍是桌面 CAD，宽窗口是主要目标。
  - Fix：在 1536 × 1024 原生窗口重跑相同状态对照后，按真实可用宽度调整轨道最小宽度和字体。

## 已验证的真实行为

- `CadWorkbenchPanel` 是唯一桌面入口；不存在旧任务中心、旧资产库、Mode、Patch、Forge 或设置页面。
- 候选 Pack 在 Blender 5.1 构建后通过 Pack 合同、10 模块导入、9 节点组合、质量检查（warning）和 Agent 重启回读。
- 原生 Tauri 可使用隔离候选 Pack/Library 启动；候选 GLB 的 API 下载 hash 与构建 Pack 一致。
- 主视口保持单一 WebGL canvas；真实 GLB 的材质、边线与相机缩放均来自运行时渲染，不是静态覆盖图。

## 比较历史

1. 旧对照显示当前工作台为深色 CAD 三栏布局，但视觉资产过暗且主体占比偏小。
2. 修复后：提高 CAD 工作室光照/曝光、提升低亮度石墨材质的可读性、缩短等轴相机距离，并引入可隔离测试的 Blender 十模块候选 Pack。
3. 最新对照仍发现上述 P1 资产精度与信息密度差距，故不将构建或截图冒充为设计通过。

## Implementation Checklist

1. 完成正式主壳体、前端、握持和顶部附件的人类美术细化，并保持稳定 Connector。
2. 将候选通过人工权属/独立审阅后，导入正式 Pack；此前保持“待审”。
3. 使用正式 Pack 在 1536 × 1024 原生工作台重新截图并完成同输入对照。
4. 仅当不存在可操作 P0/P1/P2 差异时，将本文件改为 `final result: passed`。

final result: blocked
