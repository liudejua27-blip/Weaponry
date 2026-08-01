# FGC-U004-W4 集成证据 Manifest

日期：2026-08-01
集成 worktree：`/Users/liuchongjiang/.codex/worktrees/d4c7/武神`
当前 W4 收尾前 HEAD：`8af0abd`
基线：`7758a01`

## 集成链

```text
7758a01
  → 5ec59ed  merge W1 source 96ea067
  → 0aab030  merge W2 source 65d3d8e
  → 28373ed  merge W3 source d602eb9
  → addfdc1  W4 initial evidence
  → 48e72cf → 9f94911 → 8af0abd  W3 fix chain
  → final W4 closeout docs/evidence commit
```

W2 `65d3d8e` 已验证直接父为 `7758a01`，且只有 3 个 Rust 文件。`addfdc1 → 8af0abd` 使用 `git merge --ff-only` 完成。W3 修复链只修改 CSS/smoke：`48e72cf` 两个 workbench smoke，`9f94911` 一个 workbench E2E smoke，`8af0abd` 的 `cad-workbench.css` 和同一 E2E smoke。没有真实冲突、没有整文件 `ours/theirs` 覆盖、没有修改 `main` 或 push。

## Gate 结果

状态含义：`PASS` 为当前命令事实通过；`FAIL` 为功能/验收未通过；`KNOWN FAIL` 为可定位的环境或 harness 阻断；`NOT RUN` 为没有运行或没有证据。

| 层级 | 结果 | 事实 |
| --- | --- | --- |
| W1 context/deepseek Rust focused | PASS | context 10 tests、deepseek 2 tests 通过 |
| W1 contracts/types/OpenAPI | PASS | 主环境绝对 venv 等价复验通过；schema 与 OpenAPI generated artifacts clean |
| W2 Rust/Python core checks | PASS | VP203 Rust 16、candidate capture Core 4、app-server candidate 10、ActionLoop 1、VisualConvergence、VP203/U003 Python 等价检查、OpenAPI/schema 通过 |
| W4 original U004 bridge E2E | PASS | valid GLB、local hybrid、limitation 三个 exact Rust bridge tests 通过；这是原先 W4 记录的 PASS |
| 主环境 U004 bridge 重跑 | KNOWN FAIL | `rustc` 长时间无进展后中止；不计为新的 main PASS |
| W2 PBR smoke / rendered Playwright | PASS | 主环境后续 U004 PBR smoke 与 rendered Playwright 通过 |
| W3 F025 aggregate | PASS | 完整 F025 聚合通过 |
| W3 T002 | PASS | 14/14 scenarios 通过 |
| W3 desktop typecheck/build | PASS | 主环境后续结果通过 |
| W3 F026/F006 | PASS | 主环境后续结果通过 |
| repository integrity | PASS | required paths 42、required scripts 25 |
| docs walkthrough / safety / secrets / Provider policy / schema / OpenAPI | PASS | 主环境绝对 venv 等价复验通过；docs blockers 为空、secrets matches 0、AI policy pass |
| Tauri `cargo check` | PASS | workspace native check 完成，只有既有 dead-code warnings |
| `git diff --check` | PASS | 主环境等价复验通过 |
| packaged `.app` | KNOWN FAIL | `.app` 仍被阻断，未取得 packaged GPU 八视图证据 |

## 必须分开的真实证据

| 证据 | 状态 | 结论边界 |
| --- | --- | --- |
| 真实 DeepSeek | FAIL | 历史真实 author 请求确实到达 Provider/Rust 合同边界，但没有候选 GLB/readback/capture；本轮不能把历史失败写成成功 |
| 真实千问 | NOT RUN | 本轮没有发起真实视觉比较，也没有产生真实 Qwen receipt |
| packaged app | KNOWN FAIL | `.app` 仍被阻断，未取得 packaged GPU 八视图证据；不得用本地 build/fixture 替代 |
| 真实未见输入 | NOT RUN | 没有完成正式 unseen-distribution run |
| 真人评分 | NOT RUN | 没有独立真人盲评或 `4/5` 证据 |

fixture、离线 Rust/Python smoke、确定性本地 proxy、开发 build 和截图只证明工程合同或测试 harness，不证明真实 Provider、照片级相似度、通用高质量成功或 U005 退出。

## 当前状态

W1/W2/W3 focused workstreams 已完成并集成，W4 收尾文档与证据交付完成；`FGC-U004` 必须继续 `in_progress`，`FGC-U005` 必须继续 `blocked`。退出 U004 前仍需真实 DeepSeek→GLB/readback/capture、真实千问、packaged GPU、正式未见输入和独立真人质量证据。
