# Weaponry 文档更新覆盖清单 — 2026-08-29

## 更新原则

本清单用于证明“所有文档”已被分类，而不是把历史全部改写成当前口径。

| 类别 | 更新方式 | 原因 |
| --- | --- | --- |
| active narrative/product docs | 加入 Weaponry/CrossFire 当前权威覆盖并更新核心定义 | 具有当前执行权 |
| old ADR/research/audit/proposal | 标记 superseded 或 reference-only | 保留决策历史，禁止继续驱动主线 |
| `docs/evidence/**` | 原文保留 | 历史回执不可追溯改写 |
| active Bundle `knowledge/**` | 本轮不原地修改 | 内容 hash-bound，必须新版本迁移 |
| archive/superseded Skill knowledge | 原文保留 | 历史归档不可改写 |

## 仓库盘点基线

盘点时共有 122 个 Markdown 文件：

- 68 个 narrative/proposal/仓库级 Skill 文档；
- 39 个 active hash-bound Bundle knowledge 文档；
- 10 个 archive/superseded Skill 文档；
- 5 个 evidence Markdown 文档。

本轮新增产品宪章和本覆盖清单。后续 checker 应以路径分类重新计算当前总数，不能把
新增文件误写成原始 122 基线。

ADR-0030 与刀类十天计划加入后，`scripts/check_weaponry_documentation_scope.py` 的当前分类为 126/126：

- 59 个带 current/reference/superseded 状态的 narrative 文档；
- 13 个原生 Weaponry 权威/Skill 文档；
- 39 个等待 successor version 的 hash-bound active Skill knowledge 文档；
- 10 个不可变 archive/superseded Skill 文档；
- 5 个不可变 evidence Markdown 文档；
- 0 个 unclassified Markdown。

## Active 文档统一口径

所有 active narrative 文档中的 ForgeCAD 名称解释为 Weaponry 的 Rust Runtime lineage。
所有通用 3D、机器人、科幻武器或单图 demo 描述均降级为历史能力/fixture，当前 P0 只主攻：

`高质量穿越火线非功能性游戏武器 → AuthoringMesh/Modifier → game-ready surface → FPS/engine → human acceptance`

顶部 override 只改变当前执行方向，不改变文件内部记录的历史证据数字。

## Hash-bound Skill 迁移要求

39 个 active Bundle knowledge 文件需要单独的版本化迁移任务。迁移时必须：

1. 新建 successor version，不改旧版本；
2. 把示例和 Benchmark 切换到 authorized CrossFire + original control cohort；
3. 更新 Schema、Recipe、operator lock、Validator、LICENSE/NOTICE、SBOM、provenance 和 hash；
4. 运行 registry/manifest/restart/readback Gate 后才激活；
5. 旧版本进入 archive/superseded。

在这些步骤完成前，只能写成“文档方向已更新，active Bundle 内容迁移未完成”。

## Evidence 保护

`docs/evidence/**` 的 PASS/FAIL/BLOCKED、hash、计数和时间均保持原样。新产品方向不能把：

- structural PASS 改写为 visual PASS；
- presentation-only 改写为 game-ready；
- Codex review 改写为 independent human acceptance；
- 未确认候选改写为穿越火线交付资产。

## 完成门

文档更新完成只表示产品方向、边界和执行顺序一致。它不等于 Runtime 实现、真实资产
生成、打包、引擎验证、合作方验收或发布完成。

## 2026-08-29 Gate 结果

- `python3 scripts/check_weaponry_documentation_scope.py`：PASS，126/126 已分类；
- `npm run repository:integrity`：PASS；
- `npm run release:safety-scope`：PASS；
- `npm run release:secrets-files`：PASS；
- `npm run release:license-sbom`：PASS；
- `git diff --check`：PASS；
- `npm run release:docs-walkthrough`：FAIL，原因是既有 MCP010F
  `Runtime threshold source drifted`。

最后一项必须通过重建当前 Runtime cohort 的真实 benchmark/evidence 链解除；不得修改历史
evidence hash 或放宽 checker。解除前 `WPN-DOC-001` 保持
`in_progress_blocked_by_preexisting_stage0_truth_drift`，`WPN-AUTH-001` 不得开始。
