# ADR-0023：DeepSeek / 千问唯一 AI Provider 策略

日期：2026-07-29
状态：已接受
取代：ADR-0019、ADR-0022 中允许接入第三方远程图像或神经 3D Provider 的部分

## 背景

Forge Studio 的产品目标是由中国团队建设一套类别开放、外观优先、可编辑且可审计的先进 3D 设计软件。产品不能把核心生成能力和单位经济建立在 Fal、Hunyuan 托管 Mesh API 或其他第三方聚合式生成服务上。旧 U004 曾保留一条显式付费的远程 Mesh Seed 实验路径；它没有晋级为通用 capability，也没有进入版本、Snapshot 或导出真值。

## 决策

Forge Studio 运行时的 AI Provider 只允许以下两类：

1. **DeepSeek**：当前负责文本目标理解、受限设计程序编写、工具规划和 typed patch。凭据层只接受官方 `api.deepseek.com` HTTPS 主机和 `deepseek-*` 模型家族。
2. **千问（Qwen）**：当前负责授权参考图的视觉理解、observed/inferred/unknown 证据提取和候选多视图比较。视觉凭据层只接受官方 `aliyuncs.com` HTTPS 域和 `qwen*` 模型家族。

未来可以让 DeepSeek 与千问在受检合同内互为作者或评审，但不得在没有新 ADR、代码 allowlist、迁移方案和完整 Gate 的情况下加入第三个 AI Provider。

本地 Rust Core、受限 Python Geometry Executor、Three.js renderer、确定性图像处理、几何算法、PBR 编译和用户合法导入的 GLB 不属于 AI Provider；它们继续构成实际 3D 资产生成与验证链。

## 统一运行链

```text
文字 / sealed 图片 / 当前资产
→ 千问视觉理解与证据合同
→ Rust-sealed SubjectProfile / VisualFeatureContract / RepresentationPlan
→ DeepSeek 编写受限 ForgeVisualProgram
→ Rust 校验、预算、lowering
→ 本地 RestrictedGeometryExecutor + Appearance Compiler
→ GLB/PBR/readback/固定多视图
→ 千问视觉比较
→ 最多一次 typed patch
→ 用户确认、版本化、恢复和导出
```

## 强制边界

- 永久删除 Fal 凭据、API 命令、网络适配器、恢复任务、前端入口和 live Gate；普通生成和任何实验路径都不得重新调用 Fal。
- 旧远程 Mesh Seed/Hunyuan 数据合同、迁移和 fixture 只允许为了读取历史库与回归而保留；不得注册 Tauri 命令、读取旧密钥、恢复远程任务或发起网络请求。
- `RepresentationPlan@1` 中已有的 `mesh_seed` 值暂作 schema 兼容保留，并在 capability registry 中保持 unavailable；它不证明存在可执行神经 3D Provider。
- 任意对象仍可进入开放理解；缺少本地 procedural/deformable/hybrid 表示时返回 typed limitation，不得偷偷调用其他服务，也不得替换成 C111/机械臂模板。
- Provider 输出只能提出结构化意图，不能直接拥有 GLB、版本、Snapshot、质量或导出真值。

## 验收

- `release:ai-provider-policy` 检查旧运行时文件保持删除，并扫描主程序、工作台和脚本，阻止旧 Provider 路由回归。
- Rust 凭据测试必须拒绝非 DeepSeek 主机/模型与非千问主机/模型。
- TypeScript typecheck、桌面 build、F026 单 renderer smoke 和相关 Rust Gate 必须通过。
- 不以本地 mock、schema 通过或一次 Provider 响应宣称通用 3D 质量完成；跨类别质量仍由 U004/U005 的真实 GLB、多视图和真人门决定。

## 结果

这项决策减少供应链、成本和数据边界的不确定性，但也意味着通用有机物高质量 3D 不能依靠现成远程 Mesh API 兜底。U004 必须把投入转向更强的受限设计语言、本地形变/程序化/混合表示、Appearance Compiler 和千问闭环验收；在这些能力成熟前，系统应诚实返回表示限制。
