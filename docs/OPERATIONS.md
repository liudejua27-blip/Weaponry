# ForgeCAD Runtime 运维

版本：2026-08-09
状态：单用户 MVP 运维基座；MCP004 生命周期、MCP005 reference、MCP007–009 workers/GLB 已启用，distribution release 在 MCP013

## 1. 进程

开发 MVP 运行集合是：`forgecad-runtime`、`forgecad-mcp`、ForgeCAD Viewer，以及 bounded geometry/appearance software worker（MCP008 的 fixed render 在同一受限 worker library 中执行）。first-party Skills 在 MCP006 加载；Blender worker 不属于 MVP。

Runtime 是唯一常驻产品状态写者。MCP 由 Codex 按需以 stdio 启动或连接本地 Runtime；Workers 由 Runtime 按 Job 启动。无端口 8000、FastAPI、Provider 守护进程、模型服务或常驻外部 3D API 轮询器。

开发诊断可使用 `forgecad-runtime serve`，fixture 只能由独立测试子进程局部注入；正常 `forgecad-mcp serve --stdio` 不依赖 fixture。MCP 先保持 stdio，对同一数据根的现有 `ready.json` handoff 做 authenticated probe；没有可用实例时，以短时 launcher flock 选出一个启动者，其他 MCP 会话等待并复用胜出的共享 Runtime。Runtime 失败时状态为 `Degraded`，依赖调用返回 `RUNTIME_UNAVAILABLE`，最多自动重启一次。launcher flock 只负责启动选主；Runtime 在 migration 前持有的 `runtime.writer.lock` 才是最终唯一写者。正常 MCP 适配器退出不停止已经 Ready 的 Runtime，显式 shutdown/update 才停止。正常 Runtime data dir 为 macOS `~/Library/Application Support/ForgeCAD Runtime/runtime-data`；测试才使用临时目录。MVP 不使用 TTL lease/heartbeat、daemon 或 broker。

开发包切换或升级前，可先运行 `python3 scripts/stop_forgecad_runtime.py` 做只读检查；只有确认要停止当前共享 Runtime 时才运行 `python3 scripts/stop_forgecad_runtime.py --confirm`。该脚本通过当前 `ready.json` 的本地 authenticated IPC 发送 `runtime_shutdown`，不输出 token、不向任意 PID 发信号、不删除 SQLite/CAS。停止后再完整退出并重开 Codex Desktop，MCP 才会加载新 cohort；没有 `--confirm` 时脚本不会产生运行时写入。

## 2. 启动顺序

1. 开发 MVP 验证组件合同/version/hash；正式包额外验证签名；
2. 没有可认证的 Ready handoff 时，MCP 仅用短时 launcher flock 完成复核、stale handoff 清理和启动选主；spawn 成功后立即释放 launcher flock；
3. Runtime 在 migration 前获取 OS 独占 `runtime.writer.lock`；第二实例返回 `RUNTIME_BUSY`；
4. 验证 Runtime V1 migration、SQLite integrity 和 CAS reachability；
5. MCP006 已加载十个历史 first-party development Skill Bundle；MCP010B 当前源码另加载 `primitive-blockout@0.2.0` active V2 overlay。MVP 已验证 canonical hash/trust root、Recipe DAG/单位/finite/预算、fixture receipt；MCP007 已启用 bounded geometry compiler 和 GLB readback；MCP008 已启用 appearance/fixed render；MCP009 已启用 limited quality/change/version/export；Bundle 仍只提供声明式 metadata，primitive@2 的执行仍由 Runtime/Worker 预注册 consumer 所有；
6. 非终态 Job 在 MVP 重启时转 typed failure；checkpoint 属于 MCP011；
7. 开放 authenticated local IPC 并发布可认证 handoff；launcher flock 此时已释放，最终写者仍由 `runtime.writer.lock` 判定；
8. Viewer/MCP 分别连接并读取 capabilities。

本地回归使用 `script/test_mcp004.sh`：除原有 Runtime 缺失、ready 后 crash、一次有界 restart、stdio 存活、只读无副作用和 write approval metadata 外，共享生命周期还必须覆盖 stale handoff、多个 MCP 会话、启动者 idle、passive takeover、适配器关闭后 Runtime 仍可用、未认证 idle/坏 JSON/断开客户端不阻塞合法请求，以及第二 Runtime `RUNTIME_BUSY`。正常 MCP 适配器结束不清理已经 Ready 的共享 Runtime；测试和运维通过 authenticated 显式 shutdown 清理，update 流程也可显式停止。Runtime 存活不等于未完成 Job 有 checkpoint 保证。最终源码的该回归、current `release:mvp`、同 cohort 重建、package verify、隔离 probe 与第二次 Desktop 重启后的真实工具 Gate 已 PASS；第一次失败 receipt 保留，第二次已完成并写入成功 receipt。

任一步失败，写路径保持关闭；不得启动 legacy sidecar 或打开旧 DB 回退。此前 bfa56 cohort 的第二次 Desktop 结构 Gate 已作为历史 receipt 保留；当前 d9c23b primitive-blockout 包已在用户完整 Desktop 重启后成为 live cohort，实时 `capabilities_get.build_cohort_match=true`、Runtime/doctor 为 Ready；若以后切换 cohort 仍为 false，必须按切换流程处理，不能把 Runtime Ready 单独当作 cohort 已切换。

## 3. 健康状态

Runtime 提供本地只读 health/capability 投影：版本、DB/CAS、process-lock 状态、磁盘配额、worker availability、renderer、Skill registry、Job queue、contract compatibility。不得包含 secret、绝对路径、图片、prompt 或用户内容。

状态：`healthy | degraded | read_only_recovery | incompatible | unavailable`。Viewer 和 MCP 必须显示同一状态和 digest。

## 4. 日志与审计

结构化日志只保存时间、severity、component、request/job/project opaque ID、event code、duration、byte counts 和 evidence refs。默认不保存 tool payload、自然语言、原图、绝对路径、用户名、环境变量或任意密钥。

Audit 记录永久事务、Skill 安装/撤销、恢复和导出；Job event 与 audit 分离。日志轮转和保留期可配置，删除日志不能破坏版本 lineage。

## 5. Job 操作

- 查看 queue/running/waiting/terminal 和最近事件；
- 取消只设置 durable intent，Worker acknowledge 后终态；
- 晚到结果若 cancellation/base/candidate 已失效则拒绝 admission；
- stuck Job 由 watchdog 转 failure，不重复提交永久事务；
- 重启后只从兼容 checkpoint 继续，否则明确失败。

## 6. 备份与恢复

备份顺序：暂停新永久写入 → SQLite consistent snapshot → CAS reachability manifest → Skill/asset manifests → hashes → 恢复演练。旧 Library 与新 Runtime V1 分开备份，绝不原地迁移。

## 7. 故障处置

- DB/CAS 不一致：进入 read-only recovery，先导出诊断和备份；
- 磁盘不足：拒绝新 Job/导入，不 GC confirmed objects；
- Skill hash/trust/合同失败：禁用新执行，历史仍可读；分发签名/撤销按 MCP012/013；
- Worker crash：隔离临时目录、终止/恢复 Job，不写版本；
- MCP/Viewer crash：Runtime 状态不变，重连重建投影；
- renderer unavailable：几何可诊断，但需要视觉门的 candidate 不可 confirm。

## 8. 禁止操作

不要手工编辑 SQLite/CAS、移动 version head、删除确认对象、复用旧 migration、启动端口 8000、注入模型 Key、让 MCP/Viewer 直接写库，或通过修改 QualityReport 绕过硬门。
