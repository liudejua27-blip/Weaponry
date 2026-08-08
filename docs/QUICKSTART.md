# ForgeCAD 重置期快速开始

版本：2026-08-07

## 普通用户

当前是 MCP003 诊断版，不提供生成和生产安装快速开始。Viewer 可以构建；参考导入、几何、材质和版本写入尚未开放。Runtime 的 contracts、SQLite/CAS、单写者、认证 IPC 与 MCP 只读 resources 已有本地证据，但不代表 Codex 三宿主、官方 conformance 或生产打包可用。正式可用条件见 `USER_GUIDE.md` 和 `DOCUMENTATION_STATUS.md`。

## Luna / 开发者

1. 进入仓库 `/Users/liuchongjiang/Documents/武神`；
2. 完整阅读根 `AGENTS.md`；
3. 按 `DOCUMENTATION_MAP.md` 的顺序阅读权威链；
4. 运行只读基线：

```bash
git status -sb
git diff --check
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
```

5. 从 `CODEX_TASK_INDEX.md` 领取唯一可执行任务；
6. 当前唯一任务是 `FGC-MCP003`；不要再清理未跟踪新文件；
7. 任何新的破坏性操作都必须建立新的恢复门，且不得删除 `WushenForgeLibrary`。

## MCP002 当前收尾

严格按 `CODEX_TASK_INDEX.md`：先验证 MCP002 contracts/Store/Runtime focused tests，再更新 `docs/evidence/mcp002/manifest.json` 和 handoff；不得跳到几何、材质或旧 Provider。

不要启动旧 Provider、端口 8000、FastAPI Agent 或旧工作台来验证新方向。
