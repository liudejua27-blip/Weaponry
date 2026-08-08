# ForgeCAD Runtime

ForgeCAD 正在从“内置模型与聊天的 3D Agent 工作台”硬切为“Codex 调用的本地 3D Runtime”。Codex 是大脑；ForgeCAD 提供类型化几何、UV/PBR、纹理材质、受限 Skills、渲染证据、质量编译、不可变版本、局部修改、爆炸图、回退和导出。

## 当前状态

2026-08-08：**MCP001 硬切、MCP002 Runtime 基础层和 MCP003 本地只读 MCP 适配器已通过，仍是诊断版**。

- 新方向已由 [ADR-0025](docs/ADR/0025-codex-only-mcp-3d-runtime.md) 接受；
- 旧 Provider、聊天工作台、App Server、Python Agent、旧合同和旧脚本已从当前树删除；
- 新 `forgecad-runtime`、`forgecad-mcp`、Runtime Viewer、worker protocol、Runtime V1 migration、首批 contracts、CAS、单写者和 authenticated local IPC 已通过本地 focused Gate；
- 当前版本不能宣称 Codex 已可通过 MCP 完成参考图到高质量 3D 的真实闭环；
- 当前唯一进行中的代码任务是 [FGC-MCP003](docs/CODEX_TASK_INDEX.md)：本地 resources/read、`2025-11-25` canonical / Codex `2025-06-18` compatibility 版本协商已完成；官方 conformance 与 Codex 三宿主真实连接矩阵仍待运行。

reset 前的未提交成果已归档并验证可恢复；归档不属于仓库，也不改变旧产品已经从当前树退役的决定。

## 目标体验

```text
用户在 Codex 对话并上传参考
        ↓
Codex 通过本地 MCP 调用 ForgeCAD
        ↓
ForgeCAD 编译几何、UV、PBR、纹理、材质和固定视图
        ↓
Quality Compiler 生成结构与视觉证据
        ↓
用户在 ForgeCAD Viewer 查看，在 Codex 提出局部修改/批准
        ↓
Runtime 创建不可变版本，可回退、爆炸查看和导出
```

ForgeCAD Desktop 不包含聊天、图片上传、模型选择、Provider 设置或 API Key。P0 只支持 Codex Desktop、CLI 和 IDE；ForgeCAD 不内置任何模型 SDK 或模型调用。

## 文档入口

- [文档地图](docs/DOCUMENTATION_MAP.md)
- [当前状态](docs/DOCUMENTATION_STATUS.md)
- [产品定义](docs/PRODUCT_DEFINITION.md)
- [架构设计](docs/DESIGN.md)
- [删除/迁移/升级完整清单](docs/RESET_MIGRATION_PLAN.md)
- [MCP 合同](docs/MCP_RUNTIME_CONTRACT.md)
- [Codex 集成](docs/CODEX_INTEGRATION.md)
- [编译器与质量管线](docs/COMPILER_PIPELINE.md)
- [Viewer 合同](docs/WORKBENCH_VIEWER.md)
- [Skill Package 标准](docs/SKILL_PACKAGE_STANDARD.md)
- [外部项目采用清单](docs/EXTERNAL_PROJECT_ADOPTION.md)
- [Luna 执行指南](docs/LUNA_GOAL_EXECUTION_GUIDE.md)
- [原子任务索引](docs/CODEX_TASK_INDEX.md)

## 开发规则

开始实现前必须完整阅读 [AGENTS.md](AGENTS.md)。旧 U004/Provider/Module/Mechanical 文档已失去执行权威，不能用旧测试要求恢复已废弃架构。

本项目只生成合法的非功能性视觉资产，不提供现实武器制造图、制造尺寸、材料配方、加工流程、功能机构或性能建议，也不对交通、建筑、医疗或机械结果给出安全/认证结论。

更明确地说：虚构游戏美术资产只允许非制造说明；ForgeCAD 不输出可用于现实制造武器的精确图纸。
