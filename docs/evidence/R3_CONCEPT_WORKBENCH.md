# R3 Concept Workbench Evidence

日期：2026-07-10
范围：R3 第一纵向切片；证明桌面工作台读取真实 Concept 数据和不可变 GLB，不证明正式模块资产、替换/吸附或 R3 完成。

## 已实现

- `GET /api/v1/versions/{version_id}` 返回版本自己的不可变 `WeaponConceptSpec`；
- `GET /api/v1/module-assets/{module_id}/file` 校验对象 SHA-256 后返回 immutable GLB；
- Desktop Concept API client 支持 Project、Version、Module、ModuleGraph、Variant 和 Export；
- `useConceptWorkbench` 负责加载、项目/版本切换、空状态、Starter Project 和导出；
- 视口通过 `GLTFLoader` 读取 Graph 节点引用的源 GLB，应用 node Transform 并释放 GPU 资源；
- raycast 选择同步到底部模块库、右侧节点/模块/Connector 检查器和状态栏；
- 页面不再用程序化武器模型冒充真实 ModuleGraph；没有 Graph 或 Module 时显示明确空状态；
- 当前仅可创建 `SOURCE ZIP`，GLB/OBJ/PNG 显式标记为 R5 未实现。

## 自动验证

```bash
npm run desktop:r3-concept-workbench-smoke
```

临时库中创建“寒地巡逻 S1”，注册 3 个包含真实 box mesh 的 GLB，持久化 2 条 Connector edge，将 Graph 绑定到 V2。Playwright 使用系统 Chrome 在 `1536×1024` 验证：

1. Project 与 V1/V2 来自 API；
2. 3 个模块来自 Module registry；
3. GLTFLoader 完成加载且 canvas 可用；
4. 点击前部模块后 `node_front`、模块 ID 与 `front.core` 同步；
5. 从工作台创建并下载非空 Concept ZIP；
6. 浏览器没有未处理 page error。

截图：`output/playwright/r3-concept-workbench.png`。

## 视觉核对

以用户提供的 `e9d4239c-ee36-44de-9161-5020d2fcb329.png` 为已接受结构参考，并用 `view_image` 同时检查参考图与最新截图：

- 九区高密度桌面布局保持一致；
- 顶部五阶段、左侧 Project/AI、中央视口、底部组件库、右侧检查/导出和状态栏保持一致；
- 顶部裁切与组件卡越过状态栏的问题已修复；
- 页面可见文案没有继续声称 STEP、DFM、制造就绪或当前不支持的导出格式；
- 重大剩余差异是参考图的高质量硬表面资产，而 smoke 只有 3 个程序化盒体。

## 未完成

- 首批 8–12 个高质量、UV/材质/LOD 完整的 Weapon Concept GLB；
- 模块隐藏、聚焦、拖放/替换、Connector overlay 与自动吸附；
- 保存操作、Undo/Redo、爆炸视图和 GPU 压力测试；
- combined GLB、OBJ、PNG、实际 Mesh/Assembly 检查；
- Tauri 打包窗口中的同等 E2E。
