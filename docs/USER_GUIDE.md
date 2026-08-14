# ForgeCAD 用户指南

版本：2026-08-09
当前状态：MCP005–MCP009 MVP host golden path 已完成；MCP010A Dev.app 已通过第二次 Desktop 激活 Gate；像素级相似度、真人视觉评分、完整 Desktop 3D write 和 packaged release 仍未运行

## 1. 现在能做什么

当前仓库已完成 MCP001–004 的单用户事务基座，并完成 MCP005：Runtime/IPC、candidate/approval/immutable version/restore/diagnostic export、轻量启动监督、真实 Codex CLI diagnostic write、PNG/JPEG ReferenceEvidence/CAS admission 和 Viewer read model；MCP006 完成十个历史 development-only first-party Skill Bundle，MCP010B 追加当前可执行的 `primitive-blockout@0.2.0` V2 Skill；MCP007 完成有界 typed GeometryProgram → 多 Part GLB → strict ArtifactReadback；MCP008 增加 hash-bound UV/tangent/PBR、四个固定 render pass 和 Three.js GLB canvas；MCP009 增加 limited quality、稳定 Part `change_prepare`、reject/confirm/restore 和 CAS-backed `mvp-glb` receipt。真实 Codex CLI 已完成授权图片→geometry/appearance→quality→approval/version→CAS-only GLB 的十二调用 host receipt。普通用户可以进行开发构建上的本地 MVP 评估，但仍不能把 limited evidence 宣称为像素级“高质量 3D”。Codex IDE/VS Code/Cursor/Windsurf 仍是未来兼容范围。

旧界面、内置模型和旧工作台已从当前树删除；不要寻找旧入口或配置任何模型 API Key，也不要把 reset 归档中的旧结果当作新方向的项目证据。

当前可安全做的事情：

- 保留并备份旧 Library、数据库、CAS、导入和导出；
- 阅读新产品文档和查看迁移状态；
- 当前可打开 Runtime Viewer 查看 Runtime 项目、候选、GLB bytes、UV/tangent/PBR metadata 和固定 render lineage；可通过带授权 image attachment 的真实 Codex CLI 导入 PNG/JPEG、使用 `primitive-blockout@0.2.0` 生成 V2 bounded robot、读回 ArtifactReadback/Quality 并按批准边界确认 CAS-only MVP GLB；Codex 进入设计会话时必须先读取 `ponytail-preflight@0.1.0`，随后可通过只读 `skill_list/skill_get` 查看 12 个 development-only Bundle。视觉比较和用户评分仍按 evidence 记录，不得把本地 fixture 或 limited aspect 当作相似度。

## 2. MVP 目标流程

1. 安装 ForgeCAD 和当前 P0 支持的 Codex Desktop 或 Codex CLI；
2. 安装器为 Codex 配置本地 `forgecad` MCP Server；
3. 在 Codex 对话中描述对象并上传有权使用的图片；
4. Codex 先读取 `ponytail-preflight@0.1.0`，检查是否可复用当前受限能力和最小 typed action；
5. Codex 调用 ForgeCAD 导入参考、生成候选、编译 bounded 几何/UV/tangent/PBR MaterialZone 并运行质量检查；当前没有纹理烘焙或 UDIM 交付；
6. 打开 ForgeCAD Viewer 查看 3D、部件、固定视图和质量 metadata；
7. 在 Codex 提供一个稳定 Part ID 的 `change_set`，调用 `change_prepare` 描述局部修改；当前 Viewer selection 仍是只读临时状态；
8. Codex 显示准备写入的版本摘要，用户批准后才保存；
9. 可让 Codex 准备 restore 或 `mvp-glb` CAS 导出 receipt，同样先预览再批准。完整爆炸图是 post-MVP；当前不接受任意本机导出路径。

首个 MVP host golden path 只针对一张硬表面机器人参考做 vertical slice，不承诺所有图片或类别。真实 host receipt 已通过；“参考基准质量通过”仍要有像素/轮廓指标和真人评分，完成状态以 `MVP_DELIVERY_PLAN.md` 与 `docs/evidence/mcp008|mcp009/` 为准。

ForgeCAD 内不会有聊天、图片上传、模型选择、Provider 设置或 API Key。用户只和 Codex 对话。

## 3. 版本与安全

- 候选不是保存版本；拒绝或取消不会改当前资产；
- 批准后创建不可变子版本；
- “回退”会从旧内容创建新版本，不删除历史；
- Viewer 里的相机、隐藏、隔离和临时爆炸距离默认不保存；
- `mvp-glb` 导出必须绑定已确认版本和质量/许可证清单，当前返回 CAS `output_sha256`，filesystem/package export 是后续发布门；
- 图片和资产必须由你拥有或获授权使用。

## 4. 如何判断结果可信

正式结果应同时显示：参考 hash、候选/版本 ID、几何 readback、部件树、UV/PBR/纹理检查、固定视图、参考差异、QualityReport、使用的 Skill/资产/许可证和导出 hash。

以下都不能单独证明高质量：模型能打开、贴了材质、单张好看截图、Skill 已安装、测试绿色或 Codex 说“很好”。

## 5. 限制

ForgeCAD 面向非功能性视觉资产，不提供工程/制造/结构/安全/适航/医疗/认证结论。虚构游戏或影视道具可以设计外观，但不会生成现实武器制造图、制造尺寸、材料配方、加工流程、功能机构或性能建议。

## 6. 迁移期间数据保护

旧用户 Library 不会自动删除或写入新格式。后续只通过明确的一次性离线工具导出/导入；运行前会先备份并验证 hash。若有人要求在当前脏工作树上直接删除，请停止并先阅读 `RESET_MIGRATION_PLAN.md`。
