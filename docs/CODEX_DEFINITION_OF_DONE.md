# ForgeCAD 完成定义

版本：2026-08-07
适用：所有 `FGC-MCPxxx` 任务

## 1. 原子任务 Done

任务只有同时满足以下条件才是 `done`：

- 依赖完成，修改范围没有跨下一任务；
- 退出条件逐条有当前工作树证据；
- 成功、非法输入、权限拒绝、预算、幂等、取消、重启和恢复路径按适用范围测试；
- Schema、生成类型、Runtime、MCP、Viewer、tests 和文档一致；
- 没有 legacy fallback、第二状态写者、未授权脚本/网络/路径；
- 没有 secret、prompt、原图、用户名、绝对路径或付费调用泄露；
- license/NOTICE/SBOM/provenance/signature 按任务覆盖；
- focused、aggregate、packaged、真实 Codex、视觉和真人证据分别记录；
- `git diff --check` 和相关 Gate 通过；
- 状态、能力矩阵、handoff 和用户文档同步；
- 没有把未运行或 blocked 写成通过。

## 2. MCP 工具 Done

- 公开 Schema 和 read/write annotations 正确；
- tool/resource list snapshot 固定；
- project scope、base、hash、approval、idempotency 验证；
- 错误 typed 且不泄露内部信息；
- 长任务快速返回 Job；
- Codex 实际宿主 E2E，而非自写 MCP client；
- Server/Runtime 版本不兼容、崩溃和重启 fail closed。

## 3. 永久修改 Done

- prepare 不写版本；
- 用户拒绝/超时/取消不写版本；
- hard quality fail 不可 confirm；
- 批准只创建一个不可变子版本；
- stale base 和 hash mismatch 不覆盖；
- 重复幂等请求返回同一版本；
- Viewer、snapshot、version、export 和重启 readback 同 hash；
- audit/approval/Skill/artifact lineage 完整。

## 4. 3D 质量 Done

- 原始参考和授权 evidence；
- typed design/geometry/appearance programs；
- 几何完整性、Part/source-map、严格 GLB readback；
- UV/tangent/PBR/texture/material Gate；
- 固定相机 beauty/depth/normal/AO/IDs/wireframe/UV/silhouette；
- 参考轮廓/比例/区域差异；
- Codex typed review 绑定证据；
- 跨类别独立真人盲评；
- 失败样本和限制没有从平均分隐藏；
- export/engine roundtrip 和版本一致。

结构 Gate、Skill、单张 render 或 PBR-complete GLB 不能单独满足质量 Done。

## 5. Skill Done

- 完整 Bundle 组件齐全；
- canonical hash、签名、撤销、SBOM、license/NOTICE、provenance 验证；
- Recipe DAG typed/acyclic/bounded；
- 只使用注册 Operator；
- adversarial 与资源 Gate；
- Benchmark receipt 绑定版本/hash；
- 安装、禁用、升级、回滚和历史可读；
- 每个候选仍运行 Quality Compiler。

## 6. 发布 Done

- 干净构建和签名安装包；
- Runtime/MCP/workers/Viewer/Skills 同合同版本；
- 无开发 secret/路径/环境变量；
- 新安装、升级失败回滚、数据库/CAS 备份恢复；
- Codex Desktop/CLI/IDE packaged E2E；
- Viewer 关闭仍可 compile/render/evaluate；
- 安全、内容范围、许可证和灾难恢复 Gate；
- 跨类别真人质量通过；
- 旧 Provider/Agent/workbench/8000/legacy contracts 搜索为零。

## 7. 不算完成

代码存在、类型检查通过、单元测试通过、fixture、mock、手工复制附件、开发浏览器截图、旧工作台证据、Codex 自我评价、Luna 摘要、CI 对其他 commit 绿色或“基本可用”均不单独构成完成。
