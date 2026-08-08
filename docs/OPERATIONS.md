# ForgeCAD Runtime 运维

版本：2026-08-08
状态：目标运维合同；MCP003 已提供只读 MCP resources/tools 与 Store/CAS/lease/IPC 诊断能力

## 1. 进程

发布包只包含同版本签名的：`forgecad-runtime`、`forgecad-mcp`、ForgeCAD Viewer、geometry worker、render worker、first-party Skills，以及可选经审查的 Blender worker。

Runtime 是唯一常驻产品状态写者。MCP 由 Codex 按需以 stdio 启动或连接本地 Runtime；Workers 由 Runtime 按 Job 启动。无端口 8000、FastAPI、Provider 守护进程、模型服务或常驻外部 3D API 轮询器。

## 2. 启动顺序

1. 验证安装 manifest、签名和组件合同版本；
2. 获取单实例 Runtime writer lease；
3. 验证 Runtime V1 migration、SQLite integrity 和 CAS reachability；
4. 加载 first-party Skill registry，验证签名/撤销；
5. 恢复 Job checkpoint 或转 typed failure；
6. 开放 authenticated local IPC；
7. Viewer/MCP 分别连接并读取 capabilities。

任一步失败，写路径保持关闭；不得启动 legacy sidecar 或打开旧 DB 回退。

## 3. 健康状态

Runtime 提供本地只读 health/capability 投影：版本、DB/CAS、writer lease、磁盘配额、worker availability、renderer、Skill registry、Job queue、contract compatibility。不得包含 secret、绝对路径、图片、prompt 或用户内容。

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
- Skill 签名/撤销失败：禁用新执行，历史仍可读；
- Worker crash：隔离临时目录、终止/恢复 Job，不写版本；
- MCP/Viewer crash：Runtime 状态不变，重连重建投影；
- renderer unavailable：几何可诊断，但需要视觉门的 candidate 不可 confirm。

## 8. 禁止操作

不要手工编辑 SQLite/CAS、移动 version head、删除确认对象、复用旧 migration、启动端口 8000、注入模型 Key、让 MCP/Viewer 直接写库，或通过修改 QualityReport 绕过硬门。
