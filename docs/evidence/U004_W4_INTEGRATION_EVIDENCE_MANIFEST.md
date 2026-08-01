# FGC-U004-W4 集成证据 Manifest

日期：2026-08-01
集成 worktree：`/Users/liuchongjiang/.codex/worktrees/d4c7/武神`
W4 集成前 HEAD：`28373ed`
基线：`7758a01`

## 集成链

```text
7758a01
  → 5ec59ed  merge W1 source 96ea067
  → 0aab030  merge W2 source 65d3d8e
  → 28373ed  merge W3 source d602eb9
  → final W4 docs/evidence commit
```

W2 `65d3d8e` 已验证直接父为 `7758a01`，且只有 3 个 Rust 文件。各次合并均使用普通三方合并；没有真实冲突、没有整文件 `ours/theirs` 覆盖、没有修改 `main` 或 push。

## Gate 结果

状态含义：`PASS` 为当前命令事实通过；`FAIL` 为功能/验收未通过；`KNOWN FAIL` 为可定位的环境或 harness 阻断；`NOT RUN` 为没有运行或没有证据。

| 层级 | 结果 | 事实 |
| --- | --- | --- |
| W1 context/deepseek Rust focused | PASS | context 10 tests、deepseek 2 tests 通过 |
| W1 `contracts:types:check` 聚合命令 | KNOWN FAIL | schema 生成检查通过；worktree 缺 `.venv/bin/python` |
| W2 Rust/Python core checks | PASS | VP203 Rust 16、candidate capture Core 4、app-server candidate 10、ActionLoop 1、VisualConvergence、VP203/U003 Python 等价检查、OpenAPI/schema 通过 |
| W2 `agent:vp203...`/`agent:u003...`/`agent:u004...` 聚合命令 | KNOWN FAIL | 聚合脚本硬编码 worktree `.venv/bin/python`；使用用户指定主 venv 的等价步骤通过 |
| W2 bridge E2E | PASS | valid GLB、local hybrid、limitation 三个 Rust bridge exact tests 通过 |
| W2 PBR smoke / Playwright / desktop typecheck | KNOWN FAIL | PBR smoke 对 undefined 流写入；Playwright 缺 `playwright-core`；typecheck 缺 `tsc` |
| W3 source-level F025 | PASS | parent responsibility assertion 通过 |
| W3 desktop typecheck/build | KNOWN FAIL | worktree 缺 `tsc` |
| W3 F026/F006 | KNOWN FAIL | Node 26 smoke 向 undefined 流写入 |
| W3 T002/R3 | KNOWN FAIL | worktree 缺 `playwright-core` |
| repository integrity | PASS | required paths 42、required scripts 25 |
| docs walkthrough / safety / secrets / Provider policy | PASS (equivalent) | 使用 `/Users/liuchongjiang/Documents/武神/.venv/bin/python` 等价复验；docs blockers 为空、secrets matches 0、AI policy pass |
| docs/safety/secrets/agent/contracts 聚合 npm 命令 | KNOWN FAIL | `.venv/bin/python` 不存在；schema/compile 前置部分仍通过 |
| Tauri `cargo check` | PASS | workspace native check 完成，只有既有 dead-code warnings |
| U004 universal image bridge | PASS | 3 个 Rust round-trip/limitation tests 通过 |
| U004 PBR smoke / Playwright | KNOWN FAIL | 分别为 undefined-stream 与缺 `playwright-core` |
| F025 aggregate / F026 / F006 / T002 / R3 | KNOWN FAIL | F025 静态子门通过但聚合依赖缺失；其余按 manifest 中的具体原因失败 |
| packaged QA equivalent | KNOWN FAIL | 主 venv 运行 `smoke_c111b_packaged_webgl.py`，因集成 worktree 没有 macOS `.app` 以“build the macOS .app...”退出；现有 macOS 锁屏也未解除 |
| `git diff --check` | PASS | 集成前后均通过 |

## 必须分开的真实证据

| 证据 | 状态 | 结论边界 |
| --- | --- | --- |
| 真实 DeepSeek | FAIL | 历史真实 author 请求确实到达 Provider/Rust 合同边界，但没有候选 GLB/readback/capture；本轮不能把历史失败写成成功 |
| 真实千问 | NOT RUN | 本轮没有发起真实视觉比较，也没有产生真实 Qwen receipt |
| packaged app | KNOWN FAIL | 当前集成 worktree 没有可运行 `.app`，packaged QA 未取得 GPU 八视图证据；不得用本地 build/fixture 替代 |
| 真实未见输入 | NOT RUN | 没有完成正式 unseen-distribution run |
| 真人评分 | NOT RUN | 没有独立真人盲评或 `4/5` 证据 |

fixture、离线 Rust/Python smoke、确定性本地 proxy、开发 build 和截图只证明工程合同或测试 harness，不证明真实 Provider、照片级相似度、通用高质量成功或 U005 退出。

## 当前状态

W1/W2/W3 已集成，W4 文档与证据交付完成；`FGC-U004` 继续 `in_progress`，`FGC-U005` 继续 `blocked`。退出 U004 前仍需真实 DeepSeek→GLB/readback/capture、真实千问、packaged GPU、正式未见输入和独立真人质量证据。
