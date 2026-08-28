# ForgeCAD 单用户 MVP 架构

> 2026-08-26 商业扩展边界：MVP 不改成 DCC，也不引入 Blender/Substance 作为运行时；在现有单写者基座上增加 ForgeCAD-owned authoring kernel 与隔离 typed workers。Manifold/OpenSubdiv/QuadriFlow/xatlas/Embree/MaterialX/meshoptimizer/glTF Transform 等只能按固定职责、版本、许可证、SBOM 和 receipt 采用，绝不成为第二真值。

> 2026-08-26 解释边界：MVP 单用户 host、锁、CAS、事务和回退架构是商业资产生产的基础设施，不是商业美术质量本身。Authoring/High/Low/UV/Cage-Bake 已有若干 bounded source/durable slices；完整商业执行器、artist review 与质量门仍未闭合。后续 Surface/FPS/LOD/Engine 继续作为 Runtime 管理的 fixed typed Worker/validator 接入，不能引入第二写者、任意脚本或网络服务。详见 `COMMERCIAL_GAME_WEAPON_QUALITY_PLAN.md`。

版本：2026-08-26
状态：MCP005–MCP009 单用户 MVP host golden path 已完成；视觉/packaged 证据仍单独分层

## 产品边界

ForgeCAD 是 Codex 控制的本地外部 3D 工作台。P0 只支持用户自己的 Codex Desktop/CLI 和一台 macOS 设备，不承担多用户、远程访问、后台服务治理或跨客户端并发。

~~~
Codex Desktop / CLI
        │ MCP stdio
        ▼
forgecad-mcp
  ├─ 拥有 MCP 协议会话
  ├─ 异步启动/连接同一数据根的共享 Runtime
  └─ Runtime 失败时仍保持 stdio
        │ authenticated local IPC
        ▼
forgecad-runtime
  ├─ SQLite V1 + CAS
  ├─ Project / Candidate / Job / Version
  └─ 永久状态唯一写者
        ▲
        │ read-only projection
ForgeCAD Viewer（可选）
~~~

forgecad-mcp-host 不再是产品入口。MCP 与 Runtime 的启动监督逻辑位于 forgecad-mcp 内，后端只保留 forgecad-mcp 和 forgecad-runtime 两个 executable。

## 商业生产执行器边界

MVP 的锁、CAS、事务、回退和 read model 只是商业资产生产的基础设施；当前 ForgeCAD 仍是可验证高级灰模/技术管线，缺少闭合的上游资产真值与艺术生产能力，因此不能称商业级资产生产软件。后续能力必须保持 ForgeCAD-only：固定 typed Worker 依次负责 AuthoringMesh、Native High/Low、Hero UV、Cage/Bake、Material Layer Graph、LOD/FPS；Art Director Viewer 只读呈现阶段/AOV/compare；EngineValidation 与 HeroArtReview 提供独立引擎和艺术家门。

每个 Worker 只消费 Runtime 经过审批的 typed input，返回 hash-bound artifact/readback/receipt；它不能写 SQLite/CAS、推进 Stage、运行任意脚本/DCC/网络服务或生成第二项目真值。当前机器真值仍是：唯一 `in_progress=FGC-MCP010F`，Stage=`camera-calibrated`，`secondary-form-approved=NOT_CREATED`，`FPS-HIGH-05=NOT_PASSED`，Low=`DRAFT_UNREVIEWED / structural_only / promotion_eligible=false`，proposal=`registered=false`，visual=`QUALITY_TARGET_NOT_MET`，human/engine/distribution=`NOT_RUN`，HQ360=`BLOCKED_REFERENCE_COVERAGE`，无 confirm/version/export。

Hero UV 的 7 个 registered contracts 已接入 public `hero_uv_durable_get/prepare`，Runtime drop/reopen/get **1/1 PASS** 且四个 CAS roots linked/GC；Formal High internal materializer 与 Cage/Bake fixed Worker、8-map/dilation、七记录 Store/MCP seam 也只有 source/compile/focused，完整 positive restart/public surface/current-D1 receipt 缺失。它们都不是 artist unwrap、visual、human、engine、commercial 或 packaged PASS。

## 单写者策略

MVP 不使用数据库 TTL lease、heartbeat、fencing epoch、daemon、broker 或 circuit breaker。

进程边界使用两类不同的 OS 文件锁：

~~~
runtime.sqlite
runtime.cas/
ipc/launcher.lock
runtime.writer.lock
~~~

`ipc/launcher.lock` 是短时启动选主锁：多个 MCP 适配器同时发现没有可认证的 Ready handoff 时，选主者复核/清理 stale handoff 并发起 Runtime spawn；spawn 成功后立即释放，不能把它当作数据库所有权或 Runtime 存活租约。若极端竞争产生额外 Runtime 进程，`runtime.writer.lock` 仍令失败者返回 `RUNTIME_BUSY`，且不能发布胜出的 Ready handoff。`runtime.writer.lock` 由 Runtime 在打开数据库之前取得，覆盖 migration、SQLite/CAS 初始化和 Runtime 全生命周期，是最终唯一写者硬边界。Runtime 正常退出或崩溃时由操作系统释放 writer lock。已确认的 SQLite/CAS 数据不因 MCP 或 Viewer 关闭而丢失。

## 生命周期

1. Codex 启动 forgecad-mcp serve --stdio。
2. MCP 立即进入 protocol loop，不等待 Runtime ready。
3. MCP 先对同一数据根的 `ready.json` handoff 做 authenticated probe；可连接已有 Runtime 时直接复用。
4. 没有可认证的 Ready Runtime 时，适配器竞争短时 launcher flock；选主者复核并清理 stale handoff、异步启动 Runtime，spawn 成功即释放 launcher flock。其他适配器持续复核 handoff/status，最终连接持有 writer lock 并发布 Ready 的实例。
5. Runtime 自己先取得 `runtime.writer.lock`，再进行 migration、SQLite/CAS 初始化并发布 handoff；launcher flock 不授予写权限。
6. 第一次依赖 Runtime 的调用前再次检查 ready/status。Runtime 失败时，MCP 保持 stdio，调用返回 `RUNTIME_UNAVAILABLE`。
7. Runtime Ready 前 launcher flock 已释放；任何正常 MCP 适配器会话退出都不终止共享 Runtime。只有显式 runtime shutdown/update 流程才主动停止它。
8. Runtime 意外退出时，仍存活的 MCP supervisor 最多进行一次简单重启；选主锁避免并发启动风暴，失败则进入 Degraded。
9. Runtime 可跨适配器会话存活不等于 Job 已有 checkpoint 保证；MVP 仍不承诺 Codex 断线或 Runtime 崩溃后继续未完成 Job。

这只是单用户本地进程复用，不是常驻 daemon、后台 broker 或多客户端服务治理。MCP010A 已完成；MCP010B structural source Gate 已通过但 Darwin OS memory hard cap deferred；C 为 source-focused、D/E 为 source + packaged structural，F 仍唯一 `in_progress`。各历史 Dev.app cohort receipt 只按自身范围保留，不能用 package/transport 证据替代视觉、人评或商业验收。

## 写入流程

~~~
project_create/select
→ prepare candidate
→ compile/readback/quality（MCP007–009 bounded functional core）
→ Codex 写工具审批
→ confirm
→ Runtime 原子校验 hash / head / quality / idempotency
→ immutable version + receipt
~~~

MCP004 的真实 Codex CLI 回合已创建项目并完成 contract-only candidate/confirm/restore/diagnostic export；MCP005 参考附件进入 CAS；MCP007–009 已完成 bounded geometry、GLB/readback、UV/tangent/PBR、fixed render、limited quality、stable-Part change、immutable version/restore 和 CAS `mvp-glb` receipt functional core。因此当前证据证明可开发评估的单用户 3D vertical slice，但不是像素级参考相似度、真人视觉接受或 signed packaged release。

MVP 信任边界是本机、同一用户和 Codex 宿主的 approval flow。Runtime 不建设独立的人类身份认证或密码学 approval attestation；confirm 时重新校验 candidate hash、project head、quality 和 idempotency，并由 Runtime 创建最终持久化 receipt。该 receipt 是受支持宿主流程证据，不是密码学人类签名。

## 明确延期

- launchd/SMAppService 常驻 Runtime；
- 多 Codex 客户端并发；
- Runtime heartbeat/TTL recovery/fencing（MVP 明确不需要；如未来多客户端再另立任务）；
- 远程 transport、OAuth、多租户；
- 密码学 approval attestation；
- signed/notarized release Gate（MCP013）；
- 多客户端后台 Job、第三方插件市场和通用跨类别质量。

Geometry/Render/Quality 并未延期到 MVP 之后：它们由 MCP007–009 以首个硬表面 vertical slice 实施。延期的是生产化和通用化能力。

## MVP 验收

- 基座验收：新鲜 target 同时构建 forgecad-mcp 和 forgecad-runtime；
- 无 Runtime 时 initialize 成功；
- Runtime ready 后崩溃，MCP stdio 仍存活并完成一次有界重启；共享 Runtime 的 idle owner/passive takeover 与异常未认证客户端隔离回归已在 MCP010A 最终源码 PASS，真实 Desktop attempt 2 也已 PASS；
- 第二个 Runtime 返回 RUNTIME_BUSY；
- 真实 Codex CLI 诊断回合已完成 project_create → candidate_prepare → confirm → restore → diagnostic export，并在同一临时 Runtime 上由 Tauri Viewer 读回 1 个项目、2 个版本；该 candidate 是 contract-only 非视觉对象，不代表图片、几何、生产 GLB 或签名包已完成；
- Viewer 已通过 authenticated IPC 读取项目、候选、GLB readback/bytes、版本和当前快照投影（不启动 Runtime、不打开 SQLite）；Three.js GLB/PBR canvas 已在 MCP008 完成，仍是临时只读 scene；
- Viewer 是可选只读客户端；
- 默认配置使用 forgecad-mcp，不携带测试数据根、fixture 或机器绝对路径；
- 完整 MVP 验收另要求 MCP005–009 的真实 reference hash、typed programs、真实多 Part GLB、PBR/fixed renders、局部修改、approval/version/restore/export 和用户评分，详见 `MVP_DELIVERY_PLAN.md`。
