# ForgeCAD 灾难恢复

版本：2026-08-09
状态：MCP002 已验证 focused backup/restore；完整故障注入与升级恢复在 MCP011/013

## 1. 保护对象

Runtime V1 SQLite、CAS confirmed objects、Skill/asset manifests、export manifests、release manifest 和 audit。旧 Library/DB/CAS 单独只读备份，不与新库混合。

## 2. 备份

在 Runtime 协调下创建 SQLite consistent snapshot、CAS reachability manifest、文件 hashes 和 release/contract version。备份加密和访问控制由平台策略管理；不包含临时 Job、原 attachment path、secret 或 prompt。

至少定期演练：完整备份恢复、增量对象丢失、DB 损坏、磁盘满、升级中断和整机迁移。只有实际恢复并校验 version/export hashes 才算备份可用。

## 3. 恢复顺序

1. 隔离损坏 Library，禁止写；
2. 保存诊断和只读副本；
3. 验证签名 release 组件；
4. 恢复 SQLite snapshot；
5. 恢复 CAS 并运行 reachability/hash 检查；
6. 验证 versions/snapshot/approvals/exports；
7. 非终态 Job 转明确 failure 或从兼容 checkpoint 续接；
8. 在新 Library smoke 通过后原子切换。

不得手工移动 version head、伪造 missing CAS、从 Viewer/localStorage 重建真值或启动旧 App Server。

## 4. RPO/RTO

Alpha 阶段先测量再承诺。每次 confirmed version transaction 后 WAL/CAS durability 必须完成；导出和显式备份提供额外恢复点。文档不得写未实测 RPO/RTO 数字。

## 5. 旧数据

重置前必须保存 dirty diff、untracked archive 和旧 Library manifest，并在临时目录证明可读。旧数据删除永远需要用户单独明确授权，不包含在产品代码硬切中。
