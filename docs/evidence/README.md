# ForgeCAD Evidence 入口

版本：2026-08-09
状态：MCP004 基座历史证据、MCP005 reference、MCP006 Skill Bundle、MCP007 geometry、MCP008 appearance/render、MCP009 change/version/export functional evidence 已完成；真实 Codex CLI host golden-path evidence 已加入，visual/human/packaged evidence 仍分层

## 1. 新目录

每个任务使用：

```text
docs/evidence/mcp000/
...
docs/evidence/mcp013/
```

包含 `manifest.json`、命令/exit code、环境/commit/worktree、合同/二进制/工件 hash、测试报告、脱敏日志、失败/未运行、屏幕/RenderSet/GLB/readback/quality/human evidence（适用时）。MCP005 额外记录 source/CAS hash 相等但不复制原图/绝对路径；MCP006 记录十个独立 Bundle、Recipe/DAG/operator/validator/SBOM/license/provenance/fixture receipt 状态，并明确 development-only metadata 与真实几何/渲染质量的差异；MCP007 记录 deterministic multi-Part GLB、lineage/readback、negative/no-version-on-failure、Viewer artifact projection 和真实 Codex CLI geometry/readback PASS；MCP008–009 再逐层记录 PBR、fixed render、limited reference evidence、receipt、version DAG 和用户评分。工具/Skill/外部采用的当前边界见 `../MVP_TOOL_CATALOG.md`；历史 receipt 不应被静默改写，当前状态通过新增字段或新的任务 manifest 表达。

外部项目采用 receipt 独立放在 `docs/evidence/adoption/<project>/<revision>.yaml`；出现于研究清单不是 adoption evidence。

## 2. 历史证据

U002/U004/F026 等旧 evidence 与新 Runtime 不同源，`FGC-MCP001` 从当前树删除；需要历史时从 Git/重置归档读取。不能用旧 Provider/workbench/机械 fixture 证明 Codex MCP、附件、Viewer 或新高质量闭环。

少量已经进入新任务目录、后来又被新架构明确取代的 receipt 放入 `archive/`。归档文件必须保留 `SUPERSEDED`，不在任何当前 manifest 中计为 PASS；例如 MCP004 standalone Host 收据只用于解释历史，不得恢复 `forgecad-mcp-host`。

## 3. 规则

- Markdown 总结不替代原始 receipt/artifact；`mcp004` 还需区分已通过的 restore-as-new-version/path-free diagnostic export core、local opt-in MCP wire adapter、development launcher、内置 MVP supervisor、unsigned resource placement、Codex CLI diagnostic write 与仍 BLOCKED/NOT_RUN 的 signing/notarization、Desktop/packaged write、生产文件/GLB export target；MCP009 另有真实 Codex CLI CAS-only GLB host receipt，但不覆盖这些发布门；
- PASS/FAIL/BLOCKED/NOT_RUN 分开；
- local、aggregate、packaged、真实 Codex、视觉、真人分别记录；
- evidence 引用具体 candidate/version/Skill/render/export hash；
- 不保存 secret、prompt、原图副本（用授权 CAS ref）、用户名或绝对路径；
- CI 对其他 commit 绿色不证明当前工作树。
