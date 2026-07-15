# ForgeCAD Codex 完成定义

版本：2026-07-13

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

## 2. 合同任务

- JSON Schema、Pydantic、OpenAPI 和 TypeScript 一致；
- additionalProperties/unknown field 策略明确；
- ID、枚举、数值、引用和预算有边界；
- 正向和负向 fixture；
- 兼容/破坏性变更策略；
- `contracts:types:check` 通过。

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
