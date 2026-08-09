# ForgeCAD Runtime Viewer

版本：2026-08-09
状态：MCP008–009 已实现只读 GLB canvas；MCP010F compare/selection/explosion/a11y 为 planned/unavailable，packaged WebView 仍属 MCP013

## 1. 产品角色

新桌面软件不是 Agent 工作台，而是 ForgeCAD Runtime 的可视化查看器。它回答五个问题：

1. Codex 正在处理哪个项目/候选/Job；
2. 当前 3D 结果、参考和固定视图是什么；
3. 哪些 Part、材质、UV 和质量项有问题；
4. 用户在指哪个局部；
5. 版本、恢复、爆炸图和导出证据是否一致。

Viewer 不包含聊天、prompt、图片上传、模型选择、Provider 配置、API Key、搜索、coding workspace 或 Agent timeline。

## 2. 页面模型

```text
┌ Project / Runtime / Codex connection / current version ┐
├ Assembly tree ┬──────── single 3D viewport ────────┬ Inspector ┤
│ Part hierarchy│ candidate / version / explode      │ Part      │
│ versions      │ selection / isolate / compare      │ Material  │
│ jobs          │ one WebGL context                  │ UV/Quality│
├───────────────┴─────────────────────────────────────┴───────────┤
│ Reference & fixed views | Quality issues | Job events           │
└─────────────────────────────────────────────────────────────────┘
```

只允许一个交互 WebGL renderer/context。固定视图和 AOV 是 Runtime 生成的 CAS 工件；Viewer 展示而不重新定义质量事实。

## 3. 允许的交互

- orbit/pan/zoom、视图预设、网格/线框/材质/AOV 查看；
- Part/face/source region 选择、隔离、隐藏、透明；
- reference overlay、split、flicker 和 fixed-view 对比；
- 临时 exploded distance、层级折叠和标签；
- 候选/版本切换查看、quality issue 定位、Job 取消请求；
- 复制稳定 Part ID 或让 Codex 读取当前 selection resource。

这些默认是 ephemeral，不创建资产版本。任何几何、材质、纹理、恢复、Skill 安装和导出永久动作都回到 Codex 的 prepare/approval/confirm。

## 4. 状态和一致性

Viewer 只消费 Runtime read model：`RuntimeConnection`、`ProjectSummary`、`ActiveDesignSnapshot`、`CandidateSummary`、`SelectionState`、`JobProjection`、`QualitySummary`。它没有本地版本头，不用 localStorage 恢复产品状态，不把旧请求结果覆盖新候选。

每个屏幕必须显示 project ID、version/candidate ID 和 connection freshness。Runtime 断开、Schema 不兼容或版本漂移时进入明确只读诊断状态；不能展示可以点击却会走 legacy 路径的按钮。

## 5. 当前 Viewer 与 MVP 增量

`FGC-MCP001` 已提供可编译最小 Shell，MCP004 已用 authenticated IPC 读取项目/版本/当前快照；MCP007 读取 prepared candidate、artifact SHA、GLB MIME/size、Part IDs、triangle count 和 validator status；MCP008 的 `viewer_artifact_bytes` 通过 authenticated IPC 读取受限 GLB bytes，Three.js 只建立临时 canvas scene，不写 SQLite/CAS 或第二份产品状态。

MCP007–009 已按顺序增加：

- MCP007：Runtime candidate/artifact readback、Part IDs、candidate/version/hash metadata；
- MCP008：hash-bound PBR metadata、真实 GLB canvas、固定 beauty/silhouette/normal/part-ID render evidence；
- MCP009：`quality_get`/`version_diff`、stable-ID `change_prepare` handoff、immutable version/restore 和 CAS export receipts。

Viewer 始终只读；选择是 ephemeral，永久修改回到 Codex。当前 UI 不实现真正的 Part selection/isolate、reference overlay 或 full issue editing；不能把能显示 GLB 写成 reference similarity PASS。

### 5.1 MCP010F 目标增量（当前未实现）

- reference/render split、透明 overlay、flicker、diff heatmap；
- beauty/silhouette/depth/normal/AO/part-ID/material-ID/wireframe/UV-stretch 九 AOV；
- camera lock、Part/MaterialZone selection、isolate/hide、临时 explosion；
- candidate undo/redo、issue 定位、键盘和 reduced-motion 行为。

这些 UI 必须只消费 Runtime 的 `RenderSet@2`、QualityReport 和 selection projection。屏幕图像、Three.js scene 或本地交互状态不能回写质量 PASS。源码 browser/a11y Gate 属 MCP010F；Developer ID、clean install、packaged WebView/GPU 和 packaged Codex E2E 仍属 MCP013。

## 6. 可访问性与性能

- 键盘可完成树、视图预设、issue 定位和 selection；
- 状态不只靠颜色，AOV 和 reference compare 有文字标签；
- 支持 reduced motion，爆炸动画可关闭；
- 1280×720 不横向溢出；大 Assembly 使用虚拟化；
- 交互 renderer 的卡顿不阻塞 Runtime Job；
- Renderer/WebGPU/WebGL 崩溃只影响显示，不影响候选、版本和导出真值。

## 7. 验收

- 源码中无旧 `cad-workbench` import；
- 只有一个 canvas/context；
- Viewer 关闭时完整 Runtime/MCP 流程仍成功；
- candidate、quality、selection、version 和 export 的 ID/hash 与 Runtime 一致；
- 重启恢复不依赖 localStorage；
- 当前已有 `npm run desktop:typecheck` 和 focused GLB read model evidence；源码 browser/尺寸/键盘/screen reader Gate 属 MCP010F，packaged WebView/GPU/签名安装环境属于 MCP013，当前均为 `NOT_RUN`。
