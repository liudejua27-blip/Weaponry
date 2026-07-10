# R3 Concept Workbench Evidence

日期：2026-07-10
范围：R3 当前三个纵向切片；证明真实 Concept/GLB、兼容替换和版本化桌面交互，不证明正式模块资产、完整吸附/镜像或 R3 完成。

## 已实现

- `GET /api/v1/versions/{version_id}` 返回版本自己的不可变 `WeaponConceptSpec`；
- `GET /api/v1/module-assets/{module_id}/file` 校验对象 SHA-256 后返回 immutable GLB；
- Desktop Concept API client 支持 Project、Version、Module、ModuleGraph、Variant 和 Export；
- `useConceptWorkbench` 负责加载、项目/版本切换、空状态、Starter Project 和导出；
- 视口通过 `GLTFLoader` 读取 Graph 节点引用的源 GLB，应用 node Transform 并释放 GPU 资源；
- raycast 选择同步到底部模块库、右侧节点/模块/Connector 检查器和状态栏；
- 节点支持本地隐藏/显示、聚焦和 Connector overlay；
- 组件库模块可拖到视口节点形成候选，仍需显式点击确认；
- Undo/Redo 沿 immutable parent/child Version 导航，不改写历史；
- 爆炸视图只改变展示偏移，不写回 ModuleGraph；
- 兼容模块替换走 ChangeSet preview/confirm，edge Connector 按相同 slot/type 自动重映射；
- `locked` Graph 节点由服务端保护，即使客户端省略 `protected_node_ids` 也不能绕过；
- 页面不再用程序化武器模型冒充真实 ModuleGraph；没有 Graph 或 Module 时显示明确空状态；
- 当前仅可创建 `SOURCE ZIP`，GLB/OBJ/PNG 显式标记为 R5 未实现。

## 自动验证

```bash
npm run desktop:r3-concept-workbench-smoke
```

临时库中创建“寒地巡逻 S1”，注册 4 个包含真实 box mesh 的 GLB，持久化 2 条 Connector edge，将 Graph 绑定到 V2。Playwright 使用系统 Chrome 在 `1536×1024` 验证：

1. Project 与 V1/V2 来自 API；
2. 4 个模块来自 Module registry；
3. GLTFLoader 完成加载且 canvas 可用；
4. 点击前部模块后 `node_front`、模块 ID 与 `front.core` 同步；
5. 将 `module_front_shell_01` 替换为兼容的 `module_front_shell_02`，确认后创建 V3；
6. Connector 从 `connector_front_core` 重映射为 `connector_front_alt_core`；
7. 隐藏/显示、聚焦和 overlay 控件可操作；
8. 拖拽候选后替换按钮才启用，ChangeSet 不被绕过；
9. Undo 到 V2、Redo 到 V3，并验证爆炸视图开关；
10. 从工作台创建并下载非空 Concept ZIP；
11. Agent 重启后 V3、替换模块与新 Connector 完整恢复；
12. 浏览器没有未处理 page error。

截图：`output/playwright/r3-concept-workbench.png`。

## 视觉核对

以用户提供的 `e9d4239c-ee36-44de-9161-5020d2fcb329.png` 为已接受结构参考，并用 `view_image` 同时检查参考图与最新截图：

- 九区高密度桌面布局保持一致；
- 顶部五阶段、左侧 Project/AI、中央视口、底部组件库、右侧检查/导出和状态栏保持一致；
- 顶部裁切与组件卡越过状态栏的问题已修复；
- 页面可见文案没有继续声称 STEP、DFM、制造就绪或当前不支持的导出格式；
- 重大剩余差异是参考图的高质量硬表面资产，而 smoke 只有 4 个程序化盒体。

## 未完成

- 首批 8–12 个高质量、UV/材质/LOD 完整的 Weapon Concept GLB；
- 镜像、更完整的 Connector 自动吸附与批量操作；
- 保存操作、Undo/Redo、爆炸视图和 GPU 压力测试；
- combined GLB、OBJ、PNG、实际 Mesh/Assembly 检查；
- Tauri 打包窗口中的同等 E2E。
