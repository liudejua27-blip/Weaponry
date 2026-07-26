# ADR-0018：视觉优先的远程神经 3D MVP

- 状态：Superseded by ADR-0019（历史保留；N001/N002 与 N003/N004 的通用 Provider 工程边界可复用，但远程神经 3D 不再是默认 MVP 主链）
- 日期：2026-07-26
- 决策者：项目维护者
- 取代范围：取代 ADR-0017 “必须先把所有新资产降低为 ShapeProgram 才可成为产品资产”的主生成路线；保留 ADR-0014 的 Rust 所有权、ADR-0015 的工件/视觉验收拆分、ADR-0016/0017 的 Surface Compiler、视觉收敛和受限程序化几何作为辅助能力

## 1. 决策

第一款软件收口为 **Forge Studio**：

> 面向零基础用户的 AI 3D 视觉资产生成软件。用户输入自然语言或授权图片，系统生成精致几何、完整 PBR、丰富表面细节的唯一 GLB 结果，并允许继续用自然语言修改和导出标准资产包。

仓库、内部 crate、协议兼容名暂时继续使用 `ForgeCAD`，避免一次产品改名破坏持久化、包名和现有 Gate。面向用户的命名迁移在独立任务执行。

MVP 的唯一主闭环是：

```text
用户文字 / 授权图片
→ DeepSeek 形成 VisualDesignBrief@1
→ 图像 Provider 生成单主体概念参考
→ 远程 GPU Provider 生成 PBR GLB
→ Rust 验证字节、hash、glTF/PBR 和来源
→ 固定八视角检查
→ 只展示一个未保存结果
→ 用户确认后创建不可变资产版本
→ 自然语言修改生成子版本
→ ForgeAssetPackage@1
```

第一阶段不做商城、游戏技能、战斗数值、多人游戏、精密 CAD、制造数据、手工拓扑、复杂参数面板或本地神经模型安装。

## 2. 为什么改变主路线

现有 Recipe/ShapeProgram 路线已经证明了安全编译、版本、PBR、唯一结果和恢复，但机械臂迭代 47 仍有 7 项 critical 视觉阻断。问题不是三角形数量，而是生成分布和视觉先验：固定程序化词汇很难在短期内同时解决有机过渡、中频机械层级、局部纹理和丰富微表面。

第一阶段的核心价值是“从模糊意图创造精致视觉资产”，而不是参数化 CAD。要求所有自由资产先表示为受限 ShapeProgram，会把最难的通用几何语言建设放在视觉 MVP 之前，继续延迟可见闭环。

远程神经 3D 可以让桌面端保持轻量，并把概念图到 PBR 网格的视觉生成能力提前验证。它并不取代 Rust 真值：Provider 只产生候选字节，Rust 仍拥有请求合同、授权、生命周期、取消、CAS、GLB 回读、质量、版本、导出和恢复。

## 3. 双源资产真值

`AgentAssetVersion` 后续增加受控判别字段：

```text
asset_source
├── procedural_shape_program
│   ├── ShapeProgram
│   ├── AssemblyGraph
│   └── 可执行受限局部编辑
└── neural_visual_glb
    ├── accepted GLB object hash
    ├── provider/reference lineage
    ├── PBR/readback/multiview evidence
    └── 第一阶段以重新生成子版本进行修改
```

不得为神经 GLB 伪造空 ShapeProgram、虚假 Part 或假装可参数化。两类来源共享 Project、Thread/Turn、不可变版本、ActiveDesignSnapshot、质量、唯一视口和导出，但编辑能力明确不同。

神经结果在用户确认前只是有 TTL 的候选。失败、取消、超时、迟到回调、项目切换、hash 不一致、PBR 缺失或八视角失败均不得创建版本或推进 Snapshot。

## 4. Provider 边界

DeepSeek 是视觉导演，不直接生成网格：

- 归一化用户意图；
- 判断文字/图片输入和修改范围；
- 形成概念图提示和 `VisualDesignBrief@1`；
- 选择已配置的图像/3D 后端；
- 根据确定性 readback 和八视角报告形成最多一次有针对性的修复建议。

远程 Provider 必须通过可替换端口接入。生产接入顺序由“可验证的质量 + 可调用稳定 API + GLB/PBR 合同”共同决定，而不是只按论文或 Demo 观感排序。当前首个可调用基线为 [Fal Hunyuan3D v3.1 Pro](https://fal.ai/docs/model-api-reference/3d-api/hunyuan-3d-v3.1-pro)；Pixal3D 与 TRELLIS.2 保持高优先级质量对照，在获得稳定托管 endpoint 后可晋级。Hunyuan3D-2.1 与 Stable Fast 3D 保留兼容/速度对照。这个次序是可替换路由策略，不把任一上游实现或权重打进桌面安装包，也不把公共 Gradio/ZeroGPU Demo 当产品 SLA。

请求不得包含 Provider Key、任意本地路径、数据库路径或未授权媒体。上传图片/GLB 必须先记录：

- 内容 hash；
- 支持的媒体类型；
- 用户确认拥有或获准使用；
- 用户明确同意远程处理。

单图不可见面必须标记 `ai_inferred`，不得宣传为精确重建。

Fal queue 提交必须设置 `X-Fal-Store-IO: 0`，避免把请求 JSON
保留在 Provider 历史中；同时把
`X-Fal-Object-Lifecycle-Preference` 固定为一小时，让生成媒体只在
有界下载/readback 窗口内可用，接受后的精确字节进入 Rust CAS。队列中
任务取消后不再接受迟到结果；已经运行的远端计算是否立即停止仍由上游
cancel 实现决定，不能把本机取消描述为远端必然删除。

## 5. N001 合同

Rust Core 首批冻结：

- `VisualDesignBrief@1`
- `ConceptReferenceArtifact@1`
- `Neural3DGenerationRequest@1`
- `NeuralVisualGenerationJob@1`
- `NeuralVisualArtifact@1`
- `ForgeAssetPackage@1`

状态严格按以下顺序推进：

```text
queued
→ concept_ready
→ geometry_generating
→ pbr_refining
→ glb_readback
→ multiview_review
→ ready
```

任意非终态可以 `failed` 或 `cancelled`；终态不可再迁移。开始几何生成前必须由 Rust 绑定唯一 backend 和 provider job ID。

正式神经资产至少要求：

- 非空 GLB、三角形和材质；
- Base Color、Normal、Roughness、Metallic 证据；
- GLB/CAS SHA-256；
- 概念参考 lineage；
- 恰好八张固定视图及质量报告 hash；
- 隐藏面策略。

`ForgeAssetPackage@1` 第一版严格包含：

```text
asset.glb
thumbnail.webp
turntable.mp4
manifest.json
quality-report.json
license-metadata.json
```

`asset.glb` 的 digest 必须与活动资产的已接受工件完全一致。

## 6. 自然语言修改

第一阶段不做自由网格编辑。每次修改先由 DeepSeek 路由：

| 修改 | 执行 |
| --- | --- |
| 轮廓、比例、姿态、局部结构 | 以当前 Brief/参考/GLB 为 lineage 重新生成子候选 |
| 材质、颜色 | 优先走几何保留的纹理/PBR refinement；后端不支持时重新生成 |
| 图案、磨损、发光线 | 表面 refinement；必要时复用 A005 思想但不宣称可逆 UV 编辑 |
| 完全换风格 | 新概念参考和新 3D 子候选 |

用户确认后才形成子版本；原版本始终可恢复。第一阶段的“继续修改”是可追溯再生成，不是局部 CAD 参数编辑。

## 7. MVP 退出条件

MVP 只有同时满足下列事实才完成：

1. 20 条未为其编写 Recipe 的盲测描述；
2. 覆盖机械臂、工具、无人机、虚构未来道具和工业设备；
3. 所有被标记成功的结果都是真实 GLB，并 100% 加载到同一 Three.js 视口；
4. PBR 通道由 GLB 真实回读，不从 Provider 文本推断；
5. 八视角没有严重破面、薄片、坍塌或缺失背面；
6. 至少 15/20 由独立真人在轮廓、材质可读性、细节完整度上达到 4/5；
7. 至少完成一次自然语言材质/表面修改并形成可恢复子版本；
8. 导出 `asset.glb` 与视口当前工件 hash 完全一致；
9. 从输入、远程任务、取消/失败、唯一预览、确认、重启到资产包完整 E2E；
10. 不泄露凭据，不在失败路径创建版本，不把推断背面宣传为事实。

自动 Gate、自智能体评分、概念图质量或 Provider 成功响应均不能替代以上退出条件。

## 8. 实施顺序

```text
N001 Rust 合同与纯状态机
→ N002 可替换远程 Provider 端口、假后端与任务恢复
→ N003 概念图 Provider 和授权图片入口
→ N004 真实高质量 Image-to-3D 远程适配器（首个基线 Hunyuan3D v3.1 Pro；Pixal3D/TRELLIS.2 作为可替换质量对照）
→ N005 神经 GLB 双源版本/Snapshot/CAS
→ N006 八视角/PBR/GLB 质量门
→ N007 单结果工作台与对话修改
→ N008 ForgeAssetPackage 导出
→ N009 20 Brief 盲测和人工视觉验收
```

C111A 及 C112–E004 暂停为“程序化可编辑路线”，不删除代码或证据；只有神经视觉 MVP 证明后再决定何时恢复。
