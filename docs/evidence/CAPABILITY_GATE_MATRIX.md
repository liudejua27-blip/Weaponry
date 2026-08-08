# ForgeCAD 能力—Gate 矩阵

版本：2026-08-08
状态：当前唯一能力与阻断总表

| 能力 | 当前状态 | 实现 Gate | 集成 Gate | 生产 Gate |
|---|---|---|---|---|
| 文档权威重置 | 已实现 | 新 ADR/合同/任务链 | docs/security/integrity/license PASS | 不适用 |
| 安全硬切 | 已实现 | 旧产品路径删除、contracts/viewer 骨架、Rust workspace 和 release:mcp002 | reset archive/Library hash 保留；后续 packaged hardening 在 MCP013 | clean packaged build |
| Runtime 单写者 | 已实现（MCP002 范围） | Store migration、lease、heartbeat、TTL recovery、事务 rollback | packaged kill/restart/upgrade 在 MCP011/013 | backup/upgrade/packaged |
| Runtime CAS | 已实现（MCP002 范围） | SHA-256、atomic write、capacity/missing/corruption/hash mismatch、backup/restore | reachability/GC 在 MCP011 | packaged/disaster recovery |
| authenticated local IPC | 已实现（MCP002 范围） | 0600 Unix socket、token hash/constant-time auth、错误 token 拒绝 | MCP013 packaged wiring | signed install |
| MCP stdio/resources | 部分实现（MCP003 本地范围） | MCP 2025-11-25 canonical + Codex 2025-06-18 compatibility initialize、14 个只读 tools、resources/list/read/templates、annotations、URI/contract fail closed | Codex CLI 认证只读模型回合与现代协议拒绝 PASS；官方 conformance、Codex Desktop/IDE 真实 E2E 未运行 | signed install/config |
| Codex 附件导入 | blocked | MIME/path/hash/adversarial | Desktop/CLI/IDE真实字节 | clean install E2E |
| Candidate/approval/version | 目标设计 | transaction/idempotency | reject/approve/stale/restart | packaged multi-turn |
| Geometry Compiler | 目标设计 | worker unavailable contract only | cross-category/local edit | performance/human quality |
| Appearance Compiler | 目标设计 | contract placeholder only | Viewer/headless/GLB | engine roundtrip/human |
| Render Evidence | 目标设计 | fixed view/AOV golden待 MCP009 | Viewer closed operation | cross-GPU packaged |
| Quality Compiler | 部分实现待迁移 | hard/soft layering | reference/Codex review | independent blind review |
| Skill Bundle V2 | 目标设计 | schema/DAG/operator | signature/SBOM/revocation | packaged registry/update |
| 局部修改 | 目标设计 | stable ID/change/readback待 MCP007 | Viewer selection→Codex | restart/version/export |
| 回退 | 目标设计 | restore-as-new-version | diff/restart | packaged recovery |
| 爆炸图 | 目标设计 | plan/lineage/collision | Viewer/readability | packaged/a11y |
| 导出 | 目标设计 | validator/readback待 MCP010 | version/quality/license binding | engine roundtrip |
| 外部项目/资产 | blocked | pin/license/SBOM/security | benchmark/platform | legal/distribution approval |
| 生产发布 | blocked | all MCP001–MCP012 | full Codex E2E | MCP013 + human gate |

旧 U004/Provider/F026/C111/E005 Gate 不映射为新能力 PASS。只有当前合同、当前工作树、真实运行和对应发布包证据才能升级状态。
