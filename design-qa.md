# ForgeCAD CAD 工作台设计 QA

- 日期：2026-07-12
- 参考视觉真相：`/Users/liuchongjiang/.codex/attachments/03c0589c-29dd-4070-8d2c-d30653758eff/image-1.png`
- 当前工作台截图：`artifacts/design-qa/cad-only-workbench.png`
- AI 方向交互截图：`artifacts/design-qa/cad-workbench-ai-directions.png`
- 当前 Blender 候选渲染：`artifacts/design-qa/weapon-concept-v1-full-candidate-v10-preview.png`
- TripoSR MPS 实测渲染：`artifacts/design-qa/triposr-reference-prop-mps.png`
- 实现入口：本机 Tauri `CAD 工作台.app`，`CadWorkbenchPanel`
- 截图视口：1238 × 768；状态：单一 CAD 工作区、等轴视图、真实 9 节点 ModuleGraph、网格开启。

## Findings

- [P1] 中央资产的精密硬表面密度仍明显低于参考。
  - Location：中央 Three.js 视口 / 候选 Pack。
  - Evidence：参考中的主体、前端、握持、顶部附件有连续壳体、开孔、分层轨道和材质微差；v10 已使用真实 GLB 的连续轮廓壳体、倒角、嵌合表面轨道、暗色嵌入面、视觉通风槽和紧固件，并降低大面积强调色，但整体仍以独立模块块体为主。
  - Impact：用户的首要感知仍是“模块样机”，而不是参考图级别的高精度未来概念资产。
  - Fix：继续在 `.blend` 源中人工细化主壳体/握持/顶部模块的连续曲面、开孔与面板节奏；保持相同 Module ID 和 Connector，再经 Pack、组合、质量和本机 Tauri 回归。

- [P1] 单视图 TripoSR 产物不能替代精细 CAD 候选。
  - Location：`scripts/triposr_mps_runner.py` / `triposr-reference-prop-mps.png`。
  - Evidence：在 Apple MPS 上实际完成 96 分辨率推理并导出 5,309 vertices / 10,564 faces 的 GLB，MPS 神经推理和 CPU marching-cubes 兼容层均可验证；渲染结果是粗糙重建，无法保持参考图的精确主体比例与连续硬表面。
  - Impact：图生 3D 已可作为粗略概念输入或艺术家起点，不能被默认装入正式 Module Pack。
  - Fix：仅将它保留在隔离的 rough-model 路径；正式候选继续由 Blender 源资产细化，并在视觉对照合格前保持待审。

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

- `CadWorkbenchPanel` 是唯一桌面入口；不存在旧任务中心、旧资产库、Mode、Patch、Forge 或设置页面，应用和窗口标题均为“CAD 工作台”。
- 本机 Tauri 实测：生成三条受限设计方向后，左侧助手显示 A/B/C 卡片；点击 A 仅更新主视图 Planner 预览，状态明确显示“尚未创建子版本”。
- v10 候选 Pack 在 Blender 5.1 构建后通过 Pack 合同、10 模块导入、9 节点组合、质量检查（warning）和 Agent 重启回读；总三角数 67,648，组合导出 5,191,664 bytes。它仍有 8 条 `mesh.enclosed_components` warning，不能晋级。
- `WUSHEN_TRIPOSR_RUNNER=scripts/triposr_mps_runner.py` 已通过实际 Local HTTP Runtime + Agent Adapter 手动 smoke：16.05 秒返回优化 GLB，metadata 显示真实 MPS runner；图像质量尚未达到正式候选标准。
- 原生 Tauri 可使用隔离候选 Pack/Library 启动；候选 GLB 的 API 下载 hash 与构建 Pack 一致。
- 主视口保持单一 WebGL canvas；真实 GLB 的材质、边线与相机缩放均来自运行时渲染，不是静态覆盖图。

## 比较历史

1. 旧对照显示当前工作台为深色 CAD 三栏布局，但视觉资产过暗且主体占比偏小。
2. 修复后：提高 CAD 工作室光照/曝光、提升低亮度石墨材质的可读性、缩短等轴相机距离，并将 UI 收拢为唯一 CAD 工作区。
3. 最新候选将主壳体、侧附件、下方结构和展示存储件进一步改为连续轮廓，新增暗色嵌入视觉面，收敛大面积红色；真实 Pack/组合质量 smoke 仍可执行。视觉对照仍发现上述 P1 资产精度与信息密度差距，故不将构建或截图冒充为设计通过。

## Implementation Checklist

1. 完成正式主壳体、前端、握持和顶部附件的人类美术细化，并保持稳定 Connector。
2. 将候选通过人工权属/独立审阅后，导入正式 Pack；此前保持“待审”。
3. 使用正式 Pack 在 1536 × 1024 原生工作台重新截图并完成同输入对照。
4. 仅当不存在可操作 P0/P1/P2 差异时，将本文件改为 `final result: passed`。

final result: blocked
