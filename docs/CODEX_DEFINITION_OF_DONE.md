# ForgeCAD 完成定义

版本：2026-08-13
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
- 架构/模块边界清晰，新增或废弃模块已同步 `ARCHITECTURE_MODULE_BOUNDARY.md` 和 `DEPRECATED_ISOLATION_PLAN.md`；
- 没有把未运行或 blocked 写成通过。

## 2. MCP 工具 Done

- 公开 Schema 和 read/write annotations 正确；
- tool/resource list snapshot 固定；
- project scope、base、hash、approval、idempotency 验证；
- 错误 typed 且不泄露内部信息；
- 长任务快速返回 Job；
- 按任务矩阵执行真实宿主 E2E，而非自写 MCP client：MVP vertical slice 必须真实 Codex CLI，正式发布必须 Codex Desktop + CLI；IDE/其他 Client 只有升级支持范围时才纳入；
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

## 4. 单用户 MVP functional-core Done

MCP005–MCP009 可以在 focused 本地证据全部通过后标记为 `done（功能核心）`。这一级别允许用户在开发构建中评估工具链，但不把 fixture 或自评写成参考相似度通过。

- 44 个 contracts、10 个 first-party declarative Skills、Runtime 单写者和 MCP 工具清单一致；
- 真实 PNG/JPEG admission、typed 多 Part GLB、bounded UV/tangent/PBR、fixed render、limited quality、stable-Part change、reject/confirm/restore、CAS `mvp-glb` receipt 均有当前 evidence；
- `npm run mvp:functional-core`、`npm run desktop:typecheck`、`npm run desktop:build` 和文档/安全/许可证 Gate 的结果分别记录；
- 真实 Codex MVP host golden path 已有 CLI receipt；glTF Validator、像素级 reference metrics、真人评分、packaged Viewer 和签名可以保持 `NOT_RUN/BLOCKED`，但必须在 handoff/矩阵中显式列出。

## 5. 首个硬表面参考基准 Done（MCP010F）

- MCP005–009 的退出条件全部通过，并且真实 Codex CLI 完成当前定义的十二调用 MVP host golden path；MCP010A–F 各自退出 Gate 也必须完成；
- 真实 Codex CLI 送入用户授权参考原始字节，CAS hash 一致；
- typed program 生成真实多 Part mesh/GLB，非图片平面/单盒/手工成品；
- Geometry/GLB/PBR 硬门、fixed render、reference metrics、Codex typed review 和用户评分齐全；
- stable Part ID 修改、reject/approve、immutable version、restore、export、restart 同 hash 的 focused evidence 齐全；真实 host 的 change/restore/restart 同 hash 仍需独立补证；
- 当前参考的 silhouette/landmark/region metrics、Codex typed review 和用户对 likeness/detail/material/editability 的评分均绑定同一 candidate hash；
- 单张三分之四参考通过时声明只允许 `PARTIAL_VISIBLE_VIEW_PASS`；补齐 front/back/left/right/rear-three-quarter 全身参考并逐视图通过后才允许 `HQ_360_PASS`；
- 声明限定为首个 hard-surface benchmark，不推导跨类别通用质量。

## 5.1 Agentic Design Runtime 文档 Done

仅完成 ADR/plan 文档时，只能声明目标架构已记录。要把 Agentic Design Runtime 的任一模块声明为实现，至少需要：

- 对应 Schema 进入 contracts manifest；
- Runtime producer 和 MCP read/write 边界实现；
- Viewer 或 Codex 消费路径有 focused evidence；
- 废弃/替代模块已从 active tree 移到 archive/quarantine；
- `scene_observe_get`、DesignSession、SemanticSceneGraph、Critic/Repair 等不能只靠文档或 mock 标为 PASS。

## 6. 通用 3D 质量 Done（release）

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

## 7. Skill Done

- 完整 Bundle 组件齐全；
- canonical hash、签名、撤销、SBOM、license/NOTICE、provenance 验证；
- Recipe DAG typed/acyclic/bounded；
- 只使用注册 Operator；
- adversarial 与资源 Gate；
- Benchmark receipt 绑定版本/hash；
- 安装、禁用、升级、回滚和历史可读；
- 每个候选仍运行 Quality Compiler。

MVP first-party Skill 的分发签名/在线撤销可 NOT_RUN，但 canonical hash、operator allowlist、Schema/DAG/validator/benchmark、LICENSE/NOTICE、SBOM 和 provenance 必须通过；任何可执行脚本仍禁止。

## 8. 发布 Done

- 干净构建和签名安装包；
- Runtime/MCP/workers/Viewer/Skills 同合同版本；
- 无开发 secret/路径/环境变量；
- 新安装、升级失败回滚、数据库/CAS 备份恢复；
- Codex Desktop/CLI packaged E2E；Codex IDE/VS Code/Cursor/Windsurf 不属于当前 P0 packaged Gate；
- Viewer 关闭仍可 compile/render/evaluate；
- 安全、内容范围、许可证和灾难恢复 Gate；
- 跨类别真人质量通过；
- 旧 Provider/Agent/workbench/8000/legacy contracts 搜索为零。

## 9. 不算完成

代码存在、类型检查通过、单元测试通过、fixture、mock、手工复制附件、开发浏览器截图、旧工作台证据、Codex 自我评价、Luna 摘要、CI 对其他 commit 绿色或“基本可用”均不单独构成完成。
