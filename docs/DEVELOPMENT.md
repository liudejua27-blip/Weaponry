# ForgeCAD 开发指南

版本：2026-08-08
状态：MCP003 Runtime/Store/CAS/IPC 与本地 MCP read adapter 已通过；功能仍按任务逐层开放

## 1. 目标布局

```text
crates/
  forgecad-contracts/
  forgecad-core/
  forgecad-store/
  forgecad-runtime/
  forgecad-mcp/
  forgecad-worker-protocol/
apps/
  desktop/
  geometry-worker/
  render-worker/
  blender-worker/        optional
packages/
  forgecad-contracts/
  forgecad-skills/
migrations-runtime-v1/
docs/
```

旧工作台、App Server/Protocol 和 `apps/agent` 已删除；不得重新创建这些入口。

## 2. 分层

- JSON Schema 是跨进程合同源；生成 Rust/TypeScript 类型不得手改；
- Core 是无 UI/DB/网络纯逻辑；
- Store 只向 Runtime 暴露事务 repository、CAS metadata 和 backup/restore；
- Runtime 进行 writer lease、权限、Job、审批、版本和 orchestration；
- MCP/Tauri 是 adapter；MCP 配置 Runtime socket/token 时不能直接依赖 SQLite；
- Worker 只接受 bounded internal message；
- Viewer 只读投影和 ephemeral UI state。

跨层快捷调用、第二 writer、compat fallback 和绝对路径协议会被 integrity Gate 拒绝。

## 3. 开发流程

1. 读取 AGENTS/文档链/当前任务；
2. 检查 dirty tree 和 owned paths；
3. 先写/改 Schema、negative tests 和 canonical fixtures；
4. 实现 Core/Store/Runtime；
5. 再接 MCP/Worker/Viewer；
6. 加幂等、stale、cancel、restart、budget、permission 测试；
7. 跑 focused → aggregate → packaged/真实 Codex Gate；
8. 更新状态、能力矩阵和 handoff。

## 4. 安全开发

- CSP 和 Tauri capability 只允许 Viewer 所需本地资源；
- MCP/Workers 无网络和无 broad filesystem；
- 不读取 Provider Key、Codex Key 或用户 shell 环境 secret；
- attachment/file import 必须 canonicalize、root/symlink/MIME/size/hash 检查；
- 测试 fixture 不含真实用户图片、用户名和绝对路径；
- fuzz/adversarial 输入覆盖 GLB、图片、Schema、DAG、archive 和 IPC；
- 外部依赖/资产先走采用 receipt、SBOM 和 license Gate。

## 5. Git 与脏树

不 reset、checkout 或覆盖用户修改。大删除前先取得用户授权，生成 binary-safe diff、untracked archive 和数据备份并证明恢复。除非明确要求，不 commit、push 或建 PR。

## 6. MCP002 focused baseline

```bash
CARGO_TARGET_DIR=/tmp/forgecad-mcp002-cargo-target \
  script/with_rust_toolchain.sh cargo test \
  --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --workspace --offline
```

该 Gate 覆盖 canonical hash、迁移/旧库拒绝、双 writer、TTL recovery、事务回滚、CAS corruption/capacity、备份恢复和 authenticated IPC。临时 Cargo target 不使用旧 ignored `target`。

## 7. 基线命令

当前 MCP002 收尾：

```bash
npm run release:docs-walkthrough
npm run repository:integrity
npm run release:safety-scope
npm run release:secrets-files
npm run release:license-sbom
git diff --check
```

新基线覆盖 contracts、Rust workspace、Store crash tests、worker tests、MCP conformance、Codex E2E、Viewer typecheck/build、quality/GLB、packaging、安全和 SBOM；在对应任务完成前不允许把目标能力写入用户指南。

## 7. 代码评审红线

发现内置模型/Provider、任意脚本、8000/FastAPI、MCP 直写库、Viewer 版本头、未绑定质量的 confirm、未授权路径、absolute path 输出、未 pin GitHub 代码或旧合同恢复时，评审必须拒绝。
