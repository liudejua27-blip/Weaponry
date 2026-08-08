# ForgeCAD 生产发布清单

版本：2026-08-07
当前结论：**BLOCKED，不可外部分发**

## 架构

- [x] MCP001 旧 UI/Provider/App Server/Agent/contracts/docs 已成组删除
- [x] Runtime 是唯一 DB writer，MCP/Viewer/Workers 无绕过（MCP002 focused；packaged kill/recovery 仍待后续）
- [x] 产品代码无内置模型、Provider、模型 API Key、FastAPI/8000
- [x] 新 Runtime V1 不自动打开旧 Library

## Codex/MCP

- [ ] 官方 MCP conformance
- [ ] Codex Desktop、CLI、IDE 安装/发现/连接
- [ ] 三宿主真实附件字节进入 CAS
- [ ] read/write annotations、write approval、long Job/cancel/restart
- [ ] 不以 client name 作为身份安全边界

## 3D/质量

- [ ] Geometry/Assembly/Part/source-map/readback
- [ ] UV/tangent/PBR/texture/material/bake
- [ ] headless fixed views 与全部 AOV
- [ ] reference silhouette/proportion/detail compare
- [ ] hard gates 不可绕过
- [ ] 局部修改、拒绝/批准、回退、爆炸图和导出一致
- [ ] 跨类别 Benchmark 与独立真人盲评

## 数据/恢复

- [x] SQLite/CAS single writer、事务回滚、capacity/hash/corruption focused Gate（MCP002；真实 kill-9/磁盘设备故障仍待 MCP011）
- [x] backup/restore focused Gate（MCP002；upgrade failure rollback 待 MCP013）
- [ ] 旧数据只读和一次性迁移工具
- [ ] 无绝对路径、secret、prompt、原图泄露

## 供应链/安全

- [ ] 所有 binary/Skill/asset dependency pin、LICENSE/NOTICE、SBOM、provenance、signature
- [ ] Skill 篡改/撤销/unknown Operator/DAG/budget Gate
- [ ] Worker sandbox、无任意脚本/网络/路径
- [ ] macOS signing/hardened runtime/notarization/stapling
- [ ] 安全内容范围和资产授权

## 发布体验

- [ ] clean install，无开发环境变量
- [ ] 安装器生成 Codex 配置并展示写审批
- [ ] Viewer 单 renderer、a11y、GPU fallback
- [ ] Viewer 关闭仍能 compile/render/evaluate
- [ ] 完整 reference→candidate→approval→version→restore→explode→export evidence pack

任何未勾选项都保持 BLOCKED。不得用旧工作台/Provider Gate、fixture 或内部演示抵消。
