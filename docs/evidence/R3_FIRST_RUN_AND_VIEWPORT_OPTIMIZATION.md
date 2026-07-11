# R3 首次运行闭环与视口性能优化

日期：2026-07-11

## 修复的产品断点

此前新建 Project、注册 Module Pack、验证 ModuleGraph 和绑定 Version 是分离流程，
干净库中的新项目没有 Graph，导致 Brief 方案生成被拒绝。现在工作台通过
`POST /api/v1/projects/{project_id}:initialize-workbench` 完成可重试初始化：

1. 读取内置 Pack（可用 `FORGECAD_BUNDLED_MODULE_PACK` 指定受控 Pack 路径）；
2. 注册缺失的 GLB 与 PNG thumbnail；
3. 创建并验证 9 节点默认 ModuleGraph；
4. 以不可变 V2 绑定 Graph，保留无 Graph 的初始规格 V1；
5. 允许 Brief 在旧项目无 Graph 时先完成该初始化再继续生成 A/B/C 方案。

`agent:r3-first-run-workbench-smoke` 在临时空库中验证：10 Module、9 节点、
PNG thumbnail、3 个 Brief variant 与初始化重放均通过。

## 组件库与参数边界

- Module Pack import 现在可携带并保存 `thumbnail_png_base64`；组件卡片优先读取
  `/module-assets/{module_id}/thumbnail`，旧库没有 thumbnail 时回退为类别图标；
- 整体长度、握持角度和细节密度不再以“记录本地草稿”伪装持久化，而是生成
  可确认的 ChangeSet ghost preview；
- 当前没有程序化前部拉伸能力，前部长度输入已只读并说明替代路径；
- 未完成的移动、测量、截面工具已禁用并明确说明，而非仅切换 UI 状态。

## 视口性能

`ModuleGraphViewport` 现在持久化 Renderer、Scene、Camera 与 Controls。版本、选择、
线框、Connector、质量覆盖和幽灵预览只更新对象或材质；GLB source 使用模块级缓存，
Graph 切换仅替换 module_id 真正变化的节点。

浏览器 smoke 连续在 V3/V4 间切换 20 轮的结果：

- renderer generation：`0`（此前为 `80`）；
- active WebGL context：`1`；
- canvas：`1`；
- GC 后 JS heap 增量：约 `0.71 MiB`；
- cached Pack 几何：`42`，受 `<=64` 上限保护。

## 仍明确不包含

- 截图级正式 PBR 硬表面资产、贴图和人工最终美术审核；
- TransformControls、点/角测量、裁切平面和程序化几何重建；
- B-Rep、STEP/STL、工程 BOM、DFM、切片或制造验证；
- 真实 Provider 评测、生产级异步 Worker 与跨平台 sidecar 打包。
