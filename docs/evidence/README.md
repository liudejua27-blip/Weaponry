# ForgeCAD Evidence 入口

版本：2026-08-08
状态：新 MCP 任务 evidence 规范

## 1. 新目录

每个任务使用：

```text
docs/evidence/mcp000/
...
docs/evidence/mcp013/
```

包含 `manifest.json`、命令/exit code、环境/commit/worktree、合同/二进制/工件 hash、测试报告、脱敏日志、失败/未运行、屏幕/RenderSet/AOV/GLB/readback/quality/human evidence（适用时）。`mcp002` 额外记录 migration/restart、CAS corruption/capacity、backup/restore、lease concurrency/TTL 和 authenticated IPC receipt。

## 2. 历史证据

U002/U004/F026 等旧 evidence 与新 Runtime 不同源，`FGC-MCP001` 从当前树删除；需要历史时从 Git/重置归档读取。不能用旧 Provider/workbench/机械 fixture 证明 Codex MCP、附件、Viewer 或新高质量闭环。

## 3. 规则

- Markdown 总结不替代原始 receipt/artifact；
- PASS/FAIL/BLOCKED/NOT_RUN 分开；
- local、aggregate、packaged、真实 Codex、视觉、真人分别记录；
- evidence 引用具体 candidate/version/Skill/render/export hash；
- 不保存 secret、prompt、原图副本（用授权 CAS ref）、用户名或绝对路径；
- CI 对其他 commit 绿色不证明当前工作树。
