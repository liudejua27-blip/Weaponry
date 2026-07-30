# ForgeCAD Codex 完成定义

版本：2026-07-29

任务只有满足其适用层级的全部条件才能标记为 `done`。

## 1. 所有任务共同条件

- 依赖任务已完成；
- 用户已有修改未被覆盖；
- 实现范围与任务 ID 一致；
- 没有新增无关重构；
- 没有密钥、私有绝对路径或付费调用泄漏；
- 文档区分已实现、目标、legacy 和 blocked；
- `git diff --check` 通过；
- handoff 记录真实命令和结果。
- Goal/聊天摘要没有替代任务索引、Git diff 或当次 Gate；
- `PASS / FAIL / KNOWN FAIL / NOT RUN` 分开记录，历史 PASS 不冒充当前工作区结果。
- ADR-0022 后，当前机械 Alpha、目标类别开放能力和历史回归不得互相冒充；对象类别不得被执行安全边界或模板回退替代。

## 2. 合同任务

- JSON Schema、Pydantic、OpenAPI 和 TypeScript 一致；
- additionalProperties/unknown field 策略明确；
- ID、枚举、数值、引用和预算有边界；
- 正向和负向 fixture；
- 兼容/破坏性变更策略；
- `contracts:types:check` 通过。

U002–U004 还必须证明：新版本合同不改义现有 Domain/Shape/Asset Schema；未知类别进入 capability routing 或 typed limitation；程序化、形变和 local-hybrid 进入同一 Rust-owned source/readback/version 链；运行时 AI 只使用 DeepSeek 与千问。

只增加 Schema、没有服务或测试，不代表产品能力完成。

## 3. 数据库任务

- migration 可在空库和旧库副本执行；
- migration 不重写历史数据；
- 外键、索引、唯一性和事务边界明确；
- 并发、重复请求和失败回滚测试；
- 备份/恢复覆盖新增表和对象引用；
- 数据迁移文档和回滚说明。

## 4. Agent/服务任务

- 状态机和错误码稳定；
- Idempotency-Key 和 stale base 行为；
- 取消、超时、重启或明确说明不适用；
- Provider 失败不会污染正式版本；
- API Key 不进入 Item、数据库、日志和响应；
- 单元和集成 smoke 通过。

## 5. 几何任务

- Schema/validator/runtime 同步；
- 相同输入得到相同 topology hash；
- 非有限值、非法引用、超 bounds/triangle/array/depth 在执行前拒绝；
- GLB 可解析并回读三角形、边界和材质；
- worker 失败不崩桌面或主 Agent；
- 不引入本地神经模型或任意代码执行。

## 6. 前端任务

- UI 只读取 `ActiveDesignSnapshot` 当前真值；
- loading/empty/error/stale/approval 状态完整；
- 键盘焦点、aria-live、字号和点击目标符合前端文档；
- 不增加第二个 WebGL renderer；
- typecheck、build、组件测试和相关 E2E 通过；
- 原生 Tauri 行为不能只用浏览器 smoke 代替。

## 7. 用户功能任务

- 零基础用户能理解主动作；
- 未实现技术术语默认隐藏；
- 永久修改有可见预览和确认；
- 失败说明资产是否变化以及下一步；
- 用户指南只在 E2E 通过后晋级；
- 能力—Gate 矩阵有实现位置和证据。

## 8. 发布任务

- 工作区干净，工件对应同一 commit；
- 必需 CI 绿色；
- packaged sidecar 非空且目标格式正确；
- 全新机器安装、初始化、工作台、导出和重启恢复；
- SBOM、许可证、依赖审计和资产 reviewer 完成；
- 签名、公证和回滚工件；
- `PRODUCTION_RELEASE_CHECKLIST.md` 全部必需项勾选。

只要一个必需项失败，状态必须是 `blocked`，不能使用“基本完成”“可以发布但有已知问题”等替代描述。

## 9. 视觉质量任务

- 先冻结 `VisualAcceptanceContract` 或等价受控 fixture：授权参考、must-show/must-not-show、macro/meso/micro/PBR claim、固定视图和预算；
- 每个关键结构或表面 claim 映射到真实 Part、Shape output、Material Zone、texture 和 GLB readback，不只存在于 prompt、inventory 或截图标注；
- 固定多视图来自同一 GLB 和同一工作台 renderer，不能用英雄角度掩盖其他视图；
- triangle 数、纹理分辨率、bloom、背景和模型自评分都不能单独满足质量门；
- 自动 Gate 与独立真人评分分别保留，VLM/开发模型不能代替真人；
- ADR-0021 主线的失败修复绑定同一意图和 source revision，最多一次 typed patch；旧 readback、view 和 report 正确作废；历史 v1 Gate 若仍允许两次，只作兼容回归，不扩散到 v2；
- preview→confirm→Snapshot→production export→新进程恢复保持 exact-lineage；失败、取消、stale 和篡改零永久版本副作用；
- 未见任务、参考比较、真人评分或真实 Provider 未运行时明确写 `NOT RUN`。

只有自动事实门和任务卡要求的真人门都通过，才能把对应分布标为视觉质量已实现。一个黄金工件通过不能自动证明自由生成或其他领域。

## 10. Provider、成本与商业实验

- 每次联网/计费有操作者授权、Provider/model、请求/token/图片上限、wall time、成本上限和停止条件；
- 默认无授权网络调用数为 0，失败不自动无限重试；
- Key、原始响应、私有图片和绝对路径不进入日志、Item、导出或 Git；
- 记录调用、用量、缓存命中、修复次数、端到端耗时、估算成本和固定错误分类；
- 外部输出进入同一 Rust-owned validate/readback/version 路径，不直接写 Snapshot；
- 离线 fixture、synthetic Provider 或单次成功不能替代冻结分布；
- 价格、留存、付费和单位经济只有真实试验才可标为事实，目标数字继续写 `target`；
- Luna 或其他开发模型不是产品 Provider，不进入运行时依赖、能力矩阵或用户指南。

## 11. 文档/Goal 任务

- 新决策进入 Accepted ADR，并明确它改变什么、不改变什么；
- 唯一权威归属、状态账本、计划、任务索引和 handoff 同步；
- 用户指南没有吸收尚未实现的目标能力；
- Goal 只定义持续方向，当前工作仍由一个原子任务和退出条件约束；
- 文档 Gate、仓库完整性、安全、密钥扫描和 `git diff --check` 通过；
- 只更新文档不能把运行时任务、视觉 Gate、真实 Provider、发布或商业验证标为完成。
