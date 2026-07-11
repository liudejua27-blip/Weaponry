# R3 Concept Workbench Evidence

日期：2026-07-10
范围：R3 当前七个纵向切片；证明真实 Concept/GLB、兼容替换/吸附、显式镜像、操作时间线、版本化桌面交互、浏览器资源释放和 Module Pack 工具链，不证明正式模块资产指标或 R3 完成。

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
- 当前可创建 `SOURCE ZIP`、combined `GLB`、combined `OBJ/MTL`、透明/爆炸 PNG、front/side/top、8 帧 turntable 与 render-set ZIP；同一 Version 的格式下载复用一个 Export。
- `ModulePackManifest@1` 固定包坐标、用途、许可证和模块文件索引；
- 仓库参考 Pack 含 10 个真实 GLB、九类、17 Connector、UV0/normal/三材质、512×512 缩略图与许可证；生成器 `--check` 防止二进制/Manifest/hash 漂移；
- Module Pack CLI 校验安全路径、九类 release 覆盖、GLB 2.0、UV0、材质、三角数、毫米 bounds、identity Transform、缩略图、哈希和许可证；
- CLI 默认 dry-run，显式 `--import` 后才调用 immutable Module registry，内容派生幂等键支持安全重放。
- glTF 标准米制 GLB 在 asset scene 固定换算为毫米，Graph/Connector position 统一为毫米，rotation 为 Euler XYZ 弧度；
- replace preview 在 Connector remap 后执行 rooted 子树吸附；root/child 替换、后代重定位和循环冲突拒绝均由服务端完成并进入子版本。
- `set_mirror` 将 `mirror_axis` 写入不可变 Graph 子版本，视口、Connector 吸附、检查器、Concept Export Manifest 和重启回读使用同一状态；locked 节点或 locked 后代不能被镜像/自动重定位绕过。
- 视口 cleanup 显式释放 GLTF texture、geometry/material、skeleton、controls、renderer 和 WebGL context，并暴露只读 E2E 诊断计数。
- `GET /api/v1/projects/{project_id}/change-sets` 返回权威 operation/node/status/base/result Version 与时间戳；桌面“时间线”消费该 API，并在重启后保持 replace/mirror 记录。

## 自动验证

```bash
npm run desktop:r3-concept-workbench-smoke
npm run agent:r3-module-pack-smoke
npm run agent:r3-connector-snap-smoke
npm run assets:blender-full-candidate-connector-matrix -- --pack-root <reexport-pack>
```

临时库中创建“寒地巡逻 S1”，导入仓库 10 模块参考 Pack，以 core/front/grip 组成 Graph、保留第二 front 为替换候选，将 Graph 绑定到 V2。Playwright 使用系统 Chrome 在 `1536×1024` 验证：

1. Project 与 V1/V2 来自 API；
2. 10 个参考模块来自 Module registry；
3. GLTFLoader 完成加载且 canvas 可用；
4. 点击前部模块后 `node_front`、模块 ID 与 `front.core` 同步；
5. 将 `module_front_shell_01` 替换为兼容的 `module_front_shell_02`，确认后创建 V3；
6. Connector 从 `connector_front_01_core` 重映射为 `connector_front_02_core`；
7. 隐藏/显示、聚焦和 overlay 控件可操作；
8. 拖拽候选后替换按钮才启用，ChangeSet 不被绕过；
9. Undo 到 V2、Redo 到 V3，并验证爆炸视图开关；
10. 从工作台只创建一次 Export，随后下载 Concept ZIP、combined GLB、OBJ、MTL、preview/exploded、render-set ZIP，并验证 front/top/turntable 直接工件；OBJ 含稳定节点名和 `v/vt/vn/f`，PNG 为 640×640；
11. Agent 重启后 V3、替换模块与新 Connector 完整恢复；
12. 浏览器没有未处理 page error。
13. 连续切换 V3↔V4 20 轮，始终只有 1 个 canvas/1 个 active context；参考 Pack 基准触发 80 renderer generations，GC 后 heap 增长约 3.1–3.6 MB，最终约 17 geometries/3 textures。
14. 操作时间线显示 `replace_module(node_front)`、`set_mirror(node_grip)`、confirmed 状态和结果 Version。

Module Pack smoke 另用 9 个含 triangle/UV/material 的最小 GLB 覆盖九类资产，验证 dry-run、release 门、批量注册、幂等重放和重启恢复；并确认哈希篡改、越界路径、缺许可证、跨模块重复 Connector 和 pack_id 不一致会失败。

Connector smoke 使用 100 组确定性平移、Euler XYZ 旋转、非均匀缩放和 `none/x/y/z` 镜像样本，100/100 在 `1e-7 mm / 1e-5°` 数学误差内；API 另验证 root 替换重定位所有子树、child 替换重定位后代、Connector ID remap、mirror Version/Export、幂等确认、重启恢复、locked 后代和额外循环边冲突。桌面 smoke 使用主 Connector 位于原点的正式约定，front 01→02 后保持 `[-50,0,0]` 精确吸附，并验证 grip X 镜像创建 V4、检查器显示 `mirror_axis=x`、Export 与重启保持。

2026-07-11 对十模块 Blender visual candidate 运行隔离 Connector 技术矩阵：其仅有的 front 01→02→01 两个 eligible replacement 都完成了 edge Connector remap 与 `1e-6 mm / 1e-5°` 内对齐；八个可编辑节点均完成 X 镜像、combined GLB mirror scale 与 Agent 重启回读。root core 的镜像预览被锁定策略正确以 `CHANGE_SET_INVALID` 拒绝。连续八镜像压力分支由质量规则返回 8 个 `assembly.unconnected_triangle_intersection` warning，说明 Connector 对齐和操作提交本身不保证组合无网格相交；它不是可交付组合。结果固定输出 `evidence_class=unclassified` 与 `formal_asset_evidence_eligible=false`。

合成 100% 和 candidate 2/2、8/8 都只属于技术基准。C04 的正式 ≥95% 仍需首批 10–12 个经最终许可证和独立审核批准的 Blender 模块形成替换矩阵后计算，不能用合成或待审样本替代。

GPU 数值同样只属于系统 Chrome + 最小 fixture。正式资产和 Tauri 打包窗口仍需独立 profiling；80 次 renderer generation 也说明当前状态依赖会导致额外重建，后续可优化性能，但本门已经证明 context/resource 没有随循环线性累积。

截图：`output/playwright/r3-concept-workbench.png`；镜像状态：`output/playwright/r3-concept-mirror.png`。

## 视觉核对

以用户提供的 `e9d4239c-ee36-44de-9161-5020d2fcb329.png` 为已接受结构参考，并用 `view_image` 同时检查参考图与最新截图：

- 九区高密度桌面布局保持一致；
- 顶部五阶段、左侧 Project/AI、中央视口、底部组件库、右侧检查/导出和状态栏保持一致；
- 顶部裁切与组件卡越过状态栏的问题已修复；
- 页面可见文案没有继续声称 STEP、DFM、制造就绪或当前不支持的导出格式；
- 重大剩余差异仍是参考图的人工高质量硬表面细节；当前 10 模块 Pack 是多部件、三材质程序化参考资产。

## 未完成

- 首批 10–12 个高质量、UV/材质完整的 Weapon Concept LOD0 GLB；
- 正式 10–12 个资产的 Connector 替换/镜像矩阵与 ≥95% 实测；
- 正式资产与 Tauri 窗口的 GPU profiling；
- 转台视频、正式渲染性能与最终资产视觉验收；
- Tauri 打包窗口中的同等 E2E。
