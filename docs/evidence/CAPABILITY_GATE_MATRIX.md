# ForgeCAD 能力—Gate 矩阵

日期：2026-07-11。以当前源码与自动化命令为准；概念 GLB 工作流不是工程 CAD 或制造能力。

| 产品能力 | 当前边界 | 实现位置 | 自动化证据 |
| --- | --- | --- | --- |
| 首次启动与初始项目 | 安装 Pack、创建 Project/ModuleGraph | `concept_workbench_bootstrap.py` | `npm run agent:r3-first-run-workbench-smoke` |
| 模块替换与版本 | 预览、确认、不可变版本、重启恢复 | `concept_change_sets.py` | `npm run desktop:r3-concept-workbench-smoke` |
| 人工变换 | Gizmo、坐标、吸附、ChangeSet | `ModuleGraphViewport.tsx` | Workbench E2E |
| Connector 吸附 | 父 frame 对齐、冲突与锁定拒绝 | `connector_snapping.py` | `npm run agent:r3-connector-snap-smoke` |
| 组件目录 | 缩略图、目录、搜索、审阅元数据 | `module_routes.py` | `npm run agent:r3-asset-catalog-smoke` |
| 概念质量 | Mesh/Assembly/Connector；非强度或制造结论 | `concept_quality.py` | `npm run agent:r5-mesh-assembly-quality-smoke` |
| 概念导出 | ZIP、GLB、OBJ、PNG、turntable、回读 | `concept_exports.py` | `npm run agent:r2-exports-smoke` |
| 测量与视口辅助 | 距离、表面法线夹角、版本本地标注、X-Ray、单裁切平面 | CAD Workbench | Workbench E2E |
| 工程 CAD / DFM | 未实现：B-Rep、STEP/3MF、公差、壁厚、切片、强度 | 不适用 | 禁止作为当前能力宣称 |
| 桌面发布 | 三平台 Rust 预检；签名/安装包 E2E 未完成 | `src-tauri` | GitHub `Tauri Preflight` |
| 外部许可证与 SBOM | 人工审阅仍是 release blocker | license scripts | `npm run release:license-sbom` |

## 维护规则

1. 新增 README、操作文档或 UI 可用能力时，必须更新本表并提供运行 Gate。
2. “部分实现”必须写明未支持的子能力；禁止用文案或按钮暗示完整 CAD/DFM 支持。
3. 发布评审以 GitHub 必需检查与本表命令为准；许可证审计不得跳过。
