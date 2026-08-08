# ForgeCAD Runtime Viewer

版本：2026-08-07
状态：目标设计；旧 `cad-workbench` 将整目录删除

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

## 5. 最小硬切 Viewer Shell

`FGC-MCP001` 同一提交内必须提供可编译的最小 Shell：

- 启动、窗口、单 canvas placeholder、Runtime 连接状态；
- 明确“ForgeCAD 正在迁移到 Codex MCP Runtime，当前不可生成”；
- 不含旧功能入口或兼容路由；
- Desktop typecheck/build/Tauri check 和单 renderer test 通过。

这一步允许产品功能暂时不可用，但不允许仓库故意不可编译，也不允许把 placeholder 标为 Viewer 已完成。

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
- 浏览器 smoke、packaged WebView、GPU fallback、屏幕尺寸、键盘和 screen reader Gate 均有证据。
