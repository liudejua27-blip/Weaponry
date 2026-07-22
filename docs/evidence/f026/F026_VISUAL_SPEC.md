# F026 Codex 式工作台视觉规格与开发验收

状态：开发证据，`formal_eligible=false`。本文件只证明 F026 的布局、交互与单 renderer 边界，不证明 A005、R007、V003、模型质量或真实 DeepSeek 调用已经完成。

## 冻结概念图

| 状态 | 文件 | SHA-256 |
|---|---|---|
| docked | `docs/evidence/f026/f026-docked-concept.png` | `543e3630398a1269f19c8dd7c3eaf69ad6ab0b0dbb5a609ab0a4e1a5172e21d2` |
| focus | `docs/evidence/f026/f026-focus-concept.png` | `5773758b27b74a4e13d2022cce1d7f761ae7b5c3d8c1753d3576c07230ea5e05` |

概念图仅冻结信息架构、视觉层级和交互位置。图中的机械臂、已连接模型服务和已生成结果是目标状态示例，不是当前运行证据。

## 视觉合同

- 固定左栏承载项目、对话记录和当前组件事实；中央承载 Agent 连续过程、一个结果状态槽和固定输入框；右侧持续承载唯一 Three.js 视口。
- 主色使用近黑海军蓝、低对比边界和单一钴蓝强调色；避免大面积渐变、营销卡片和第二套编辑器视觉语言。
- 正文以 11–13px 的高密度桌面排版为主，关键状态用普通中文表达；不显示方向编号、内部变体 ID、技术 Schema 或评分排名。
- `+` 菜单只提供风格、材质和参考入口。F026 阶段如实标注参考图能力等待 R007，不能把 GLB 导入冒充参考图重建。
- `docked → focus → docked` 只移动同一个视口 DOM；保持相机、选择、Snapshot render preset、renderer generation 与 WebGL context，不创建第二个 canvas。
- F026 兼容层可以从旧 Planner 响应中只取第一条文本方向来启动一次 build，但 UI 不能显示方向选择。这个兼容层不是 V003 的最终单次生成合同。

## 浏览器开发验收

方法：通过 Chrome Browser 插件直接控制本地 Vite 工作台 `http://127.0.0.1:1420/#/cad`，使用真实 DOM、键盘和 Three.js canvas；未使用图片热区或脚本注入替代应用交互。检查了 1536×960 桌面尺寸与 1180×760 最小支持尺寸。

证据文件：

- `output/f026-visual-qa/f026-docked-final-1536x960.jpg`
- `output/f026-visual-qa/f026-focus-final-1536x960.jpg`
- `output/f026-visual-qa/f026-docked-final-1180x760.jpg`
- `output/f026-visual-qa/f026-plus-menu-final-1180x760.jpg`

实测事实：

- 1536×960 docked：canvas `1`，renderer generation `2`，active WebGL contexts `1`，视口宽约 `614px`。
- 1536×960 focus：canvas `1`，renderer generation `2`，active WebGL contexts `1`，同一视口宽约 `968px`。
- Escape 返回 docked 后 canvas、generation、context 仍为 `1 / 2 / 1`，焦点回到“放大 3D 视图”。
- 1180×760：document 横向溢出 `0px`，中央时间线横向溢出 `0px`，结果状态槽 `1`，方向选择器 `0`。
- `+` 菜单实际显示“选择风格 / 选择材质 / 参考图或 GLB”，并明确提示“当前仅兼容 GLB；参考图引导重建待 R007”。

## 概念图与实现对照

| 对照点 | 冻结概念 | 当前实现 | 结论 |
|---|---|---|---|
| 信息架构 | 左栏、中央会话、右侧 3D、底部输入 | 三个区域和固定输入均存在 | 一致 |
| 默认 3D 布局 | 3D 永久停靠右侧 | 唯一 canvas 停靠右侧 | 一致 |
| Focus 布局 | 3D 移到中央，会话变窄 | CSS grid 重排同一视口，会话成为右侧窄栏 | 一致 |
| 视觉语言 | 深海军蓝、细边界、钴蓝强调 | 色板、密度和控件层级相符 | 一致 |
| 单一结果 | 中央只展示“唯一结果” | 只有一个 `GenerationResultCard`，没有三方向卡或 `/ 3 版` | 一致 |
| 输入与附件 | 底部固定输入，`+` 提供三类入口 | 固定 composer 与三类菜单已实现 | 一致；参考图入口仍等待 R007 |
| 运行内容 | 示例为已生成机械臂和已连接 Provider | 本次开发环境为空项目、离线 Provider，兼容 Python 路由返回 HTTP 410 | 状态差异；不属于视觉伪造，不能用概念图替代运行证据 |

## 首屏文案差异

实现没有出现“Agent 完整外观方向”“当前第 N / 3 版”或“换一版外观”。空状态使用“从左侧开始新设计”“等待生成”，并显示真实离线/错误状态。与概念图中已完成机械臂的文案差异来自测试数据和运行时状态，不能为了截图而硬编码成功结果。
