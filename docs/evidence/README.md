# ForgeCAD Evidence 入口

版本：2026-08-13
状态：MCP004 基座历史证据、MCP005 reference、MCP006 Skill Bundle、MCP007 geometry、MCP008 appearance/render、MCP009 change/version/export functional evidence 已完成；真实 Codex CLI host golden-path evidence 已加入，visual/human/packaged evidence 仍分层。ADR-0026 的目标架构证据尚未产生；三个 superseded Skill provenance 已移入 `packages/forgecad-skills/archive/`。

## 1. 新目录

MCP010F 当前 CADFit/Boolean/Visual Surface 的分层状态由 `docs/evidence/mcp010f/current-quality-evidence-ledger.json` 和 `scripts/check_mcp010f_current_quality_evidence.py` 维护：当前 source cohort、历史 provisional observation、无 cohort 的隔离 Boolean 回执、Manifold adoption determinism/resource/negative Gate、Visual Surface readback、bounded mesh-derived surface analysis、`surface-patch@1` open-surface、`surface-shell@1` bounded shell、`subd-cage@1` editable regular-quad cage source Gate、以及 ActionRun/restart 回执不得交叉借字段。Surface analysis receipt 只证明同一 candidate GLB 的 typed readback，不证明 arbitrary-topology SubD/NURBS principal curvature、完整 surface backend 或视觉质量；SubD catalog activation 后，既有真实视觉 transport receipt 仍属于旧 source cohort，必须重跑后才能作为当前 binary 证据。

每个任务使用：

```text
docs/evidence/mcp000/
...
docs/evidence/mcp013/
```

包含 `manifest.json`、命令/exit code、环境/commit/worktree、合同/二进制/工件 hash、测试报告、脱敏日志、失败/未运行、屏幕/RenderSet/GLB/readback/quality/human evidence（适用时）。MCP005 额外记录 source/CAS hash 相等但不复制原图/绝对路径；MCP006 记录十个独立 Bundle、Recipe/DAG/operator/validator/SBOM/license/provenance/fixture receipt 状态，并明确 development-only metadata 与真实几何/渲染质量的差异；MCP007 记录 deterministic multi-Part GLB、lineage/readback、negative/no-version-on-failure、Viewer artifact projection 和真实 Codex CLI geometry/readback PASS；MCP008–009 再逐层记录 PBR、fixed render、limited reference evidence、receipt、version DAG 和用户评分。工具/Skill/外部采用的当前边界见 `../MVP_TOOL_CATALOG.md`；历史 receipt 不应被静默改写，当前状态通过新增字段或新的任务 manifest 表达。

外部项目 receipt 独立放在 `docs/evidence/adoption/<project>/<revision>.yaml`。`approval: research-authorized` 只固定授权研究的 revision、许可证和候选文件，不构成 adoption evidence、SBOM entry 或可分发代码；只有 `approval: accepted` 才允许进入 lockfile/package。Manifold 当前是例外：其 fixed revision 以 product-owned isolated Worker 形式 accepted，仍不作为通用 Runtime dependency；build123d、BlenderMCP、CadQuery、Manifold、MaterialX 的当前研究规则见 `../LUNA_GITHUB_REPLICATION_PLAYBOOK.md`。

## 2. 历史证据

U002/U004/F026 等旧 evidence 与新 Runtime 不同源，`FGC-MCP001` 从当前树删除；需要历史时从 Git/重置归档读取。不能用旧 Provider/workbench/机械 fixture 证明 Codex MCP、附件、Viewer 或新高质量闭环。

少量已经进入新任务目录、后来又被新架构明确取代的 receipt 放入 `archive/`。归档文件必须保留 `SUPERSEDED`，不在任何当前 manifest 中计为 PASS；例如 MCP004 standalone Host 收据只用于解释历史，不得恢复 `forgecad-mcp-host`。

Skill provenance 若已被替代但仍需保留，不放在 active Skill 根目录；放入 `packages/forgecad-skills/archive/**`，由 archive manifest 保存 pre-archive tree hash，并不得出现在 `registry.json`、Runtime build archive 或当前 evidence manifest 的 active Skill 计数中。

## 3. 规则

- Markdown 总结不替代原始 receipt/artifact；`mcp004` 还需区分已通过的 restore-as-new-version/path-free diagnostic export core、local opt-in MCP wire adapter、development launcher、内置 MVP supervisor、unsigned resource placement、Codex CLI diagnostic write 与仍 BLOCKED/NOT_RUN 的 signing/notarization、Desktop/packaged write、生产文件/GLB export target；MCP009 另有真实 Codex CLI CAS-only GLB host receipt，但不覆盖这些发布门；
- PASS/FAIL/BLOCKED/NOT_RUN 分开；
- local、aggregate、packaged、真实 Codex、视觉、真人分别记录；
- evidence 引用具体 candidate/version/Skill/render/export hash；
- 不保存 secret、prompt、原图副本（用授权 CAS ref）、用户名或绝对路径；
- CI 对其他 commit 绿色不证明当前工作树。
