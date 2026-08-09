# ForgeCAD 能力—Gate 矩阵

版本：2026-08-09
状态：当前唯一能力与阻断总表

| 能力 | 当前状态 | 实现 Gate | 集成 Gate | 生产 Gate |
|---|---|---|---|---|
| 文档权威重置 / Dev 激活 | MCP010A in_progress / Desktop attempt 1 FAIL / attempt 2 NOT_RUN | MCP010A–F 权威链、真实 Codex CLI、config activation、共享 Runtime/IPC 修复 tests 与 current `release:mvp` PASS；显式 write opt-in 已配置 | 第一次 Desktop 重启只列出 17 个只读工具，Runtime 不可用且未创建项目；cohort `7a8fddf99c57893db93fe1bdd98ab65302bd890d191026495cbbc63ae4652064` 重建安装、ad-hoc deep-strict、package verify、isolated Ready/cohort-match/project probe PASS，第二次重启 `NOT_RUN` | 不适用 |
| 安全硬切 | 已实现 | 旧产品路径删除、contracts/viewer 骨架、Rust workspace 和 release:mcp002 | reset archive/Library hash 保留；后续 packaged hardening 在 MCP013 | clean packaged build |
| Runtime 单写者 | 已实现（MCP002）；共享启动协调修复 PASS | migration 前 OS 独占 `runtime.writer.lock`、第二实例 `RUNTIME_BUSY`、事务 rollback 是已有 Gate；短时 launcher flock 只做选主，不能替代 writer lock | normal adapter exit 后复用 Ready Runtime、idle owner/passive takeover、rogue pre-auth client 隔离回归 PASS；第二次 Desktop live Gate `NOT_RUN`，packaged kill-9/upgrade 在 MCP011/013 | backup/upgrade/packaged |
| Runtime CAS | 已实现（MCP002 范围） | SHA-256、atomic write、capacity/missing/corruption/hash mismatch、backup/restore | reachability/GC 在 MCP011 | packaged/disaster recovery |
| authenticated local IPC | 已实现（MCP002 范围） | 0600 Unix socket、token hash/constant-time auth、错误 token 拒绝 | MCP013 packaged wiring | signed install |
| MCP stdio/resources | 已实现（MVP host golden path）；MCP010A shared lifecycle remediation PASS | MCP 2025-11-25 canonical + Codex 2025-06-18 compatibility initialize、默认 17 个只读 tools；authenticated/dynamic IPC + explicit env opt-in 时 30 个 tools（17 read + 13 write，含 MCP009 change）；内置 supervisor、Starting/Ready/Degraded/Restarting、Runtime 缺失/崩溃时 stdio survival、resources/list/read/templates、annotations、URI/contract fail closed | Codex Desktop/CLI discovery、connection、read-only E2E 与真实 Codex CLI 3D 主链 PASS；MCP010A Desktop attempt 1 因 stale handoff/无活 Runtime/write opt-in 未继承而 FAIL，写事务未运行；最终修复测试、新包与隔离 probe PASS，attempt 2 `NOT_RUN`；IDE/其他 Client 非阻塞未来范围 | signed install/config |
| Codex 附件导入 | 已实现（MCP005，CLI） | PNG/JPEG MIME/path/hash/decode-limit/adversarial | 真实 Codex CLI bytes/CAS readback PASS；Desktop bridge `NOT_RUN / unavailable` | clean install E2E |
| Candidate/approval/version | 已实现（MVP host golden path） | prepare/Job/approval/confirm/reject/restore-as-new-version/diagnostic-export/idempotency/stale/hash/quality/expiry、MCP009 stable-Part `change_prepare` PASS | 真实 Codex CLI candidate/quality/version/CAS export PASS；MCP010A Desktop attempt 1 在 write tool 可见性前 FAIL、无项目写入，故 Desktop 写事务仍 NOT_RUN；change/restore/restart hash 与 packaged 待后续 Gate | packaged multi-turn |
| Geometry Compiler | 部分实现（MCP007 done） | canonical GeometryProgram；bounded box/cylinder/sphere；finite/index/budget/lineage；deterministic GLB/readback；unknown operator/no-version-on-failure PASS | Runtime/MCP authenticated geometry prepare + Viewer candidate/artifact read model PASS；real Codex CLI geometry/readback slice PASS（14 parts/516 triangles）；MCP009 full host appearance链已单独 PASS | full profile/boolean/bevel, local edit, cross-category/performance/human |
| Geometry V2 / detail | 目标设计（MCP010B/D blocked） | `GeometryProgram@2`/OperatorCatalog/真实 GLB readback；profile/revolve/sweep/loft/mirror/array/macros | Manifold 仅 adoption accepted 后进入隔离 Worker；当前 unavailable | cross-category/performance |
| Appearance Compiler | 已实现（MCP008 bounded） | hash-bound UV/tangent/PBR/MaterialZone/GLB readback PASS；完整 glTF validator/texture bake NOT_RUN | Runtime fixed artifact + Viewer canvas PASS；真实 Codex appearance/readback 在 MCP009 receipt PASS | engine roundtrip/human |
| 离线 AssetPack / PBR V2 | 目标设计（MCP010E blocked） | first-party pack、逐资产 provenance、xatlas/mikktspace、颜色空间/纹理预算、无 external URI | 当前无 pack/import tool，Runtime 不联网；generic lifecycle 属 MCP012 | signed package/license review |
| Render Evidence | 已实现（MCP008 bounded） | deterministic beauty/silhouette/normal/part-ID PNG + camera/renderer hash PASS | Viewer closed operation focused PASS | full AOV/cross-GPU packaged |
| Reference compare V2 | 目标设计（MCP010C blocked） | perspective/z-buffer、九 AOV、mask/IoU/F1/landmark/region、typed review/human receipt | 当前 tools 仍 17 read + 13 write；目标 18 + 16 未暴露 | cross-category blind review |
| Quality Compiler | 部分实现（MCP009 limited） | candidate/GLB/PBR/fixed-render checks PASS；reference aspect comparison explicitly limited | `quality_get`/`version_diff` and functional tests PASS；typed visual review/real Codex/user score NOT_RUN | independent cross-category blind review |
| Skill Bundle | MCP006 done（development-only） | 44 typed contracts、十项 first-party registry、十个独立 Bundle、canonical/trust hash、operator/validator allowlist、Recipe DAG/单位/finite/预算、合成正/负 fixture receipt、LICENSE/NOTICE/SPDX SBOM/provenance、无网络/无动态代码/无模型调用 | Bundle/Recipe focused PASS；geometry/appearance consumers MCP007–009 PASS；visual benchmark NOT_RUN | distribution signature/revocation/third-party registry |
| 局部修改 | 已实现（MVP bounded） | stable ID + allowlisted `change_prepare` + base head/readback PASS；不声称 mesh-delta | Runtime focused PASS；真实 Codex/Viewer selection E2E NOT_RUN | full undo/explode/a11y |
| 回退 | 已实现（MVP） | restore-as-new-version PASS | Runtime focused PASS；restart/packaged recovery NOT_RUN | packaged recovery |
| 爆炸图 | 目标设计 | plan/lineage/collision | Viewer/readability | packaged/a11y |
| 导出 | 已实现（CAS-backed MVP GLB） | `manifest-json/diagnostic` 与 `glb/mvp-glb` prepare/confirm；confirmed quality GLB、output hash、approval/idempotency PASS；不写任意路径 | MCP009 functional PASS；filesystem/package target NOT_RUN | engine roundtrip |
| 外部项目/资产 | evaluation allowed | adoption receipt/pin/license/SBOM/security | MVP benchmark before `accepted` | legal/distribution approval |
| 首个硬表面 MVP | host golden path PASS / MCP010A attempt 1 FAIL / visual benchmark OPEN | MCP005–MCP009 focused PASS；MCP010A remediation/rebuild/package/isolated probe PASS；010A 保持 in_progress、010B–F blocked | real Codex CLI full chain PASS；Desktop attempt 2 `NOT_RUN`；单图最高 `PARTIAL_VISIBLE_VIEW_PASS`，360 `BLOCKED_REFERENCE_COVERAGE` | 不要求签名分发 |
| 生产发布 | blocked | MCP001–MCP012 | full packaged Codex E2E | MCP013 + cross-category human gate |

旧 U004/Provider/F026/C111/E005 Gate 不映射为新能力 PASS。只有当前合同、当前工作树、真实运行和对应发布包证据才能升级状态。
