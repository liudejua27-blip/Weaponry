# ForgeCAD 发布维护

版本：2026-08-09
状态：MCP013 后的目标流程；不阻塞开发 MVP

## 1. 版本集合

每次 release 固定 Viewer、Runtime、MCP、workers、contracts、DB schema、Skills 和 asset packs 的版本/hash。release manifest 是分发真值；单独替换组件会关闭写路径。

## 2. 发布分支

从干净、已通过 MCP013 的 commit 构建。CI 对目标 commit 绿色不证明本机脏工作树。构建产物、SBOM、NOTICE、provenance、signatures、Codex E2E 和 quality/human evidence 绑定 commit。

## 3. 更新类型

- patch：合同兼容 bug/security 修复；
- minor：向后兼容能力/Skill；
- major：合同或 DB 破坏性变更，需 ADR、迁移和 rollback；
- Skill/asset revocation：独立签名 registry 更新，不改历史版本 receipt。

## 4. 维护 Gate

依赖更新先看 changelog/license/SBOM/security/API，再在隔离分支跑全 Gate。不得使用浮动 Git revision 或未经审查的自动下载。Renderer/codec/geometry 依赖变化必须重跑 golden、readback、跨平台和真人基线。

## 5. 回滚

升级前备份；失败原子恢复旧二进制和兼容数据库快照。已经用新 Schema 写入的数据不能直接由旧 Runtime打开时，进入 read-only recovery并提供导出，而不是强行 downgrade。

## 6. 支持策略

发布说明区分实现、已测宿主/平台、已知限制和未通过质量类别。安全/许可证/Skill 撤销有紧急通道；不静默改变用户资产或版本历史。
