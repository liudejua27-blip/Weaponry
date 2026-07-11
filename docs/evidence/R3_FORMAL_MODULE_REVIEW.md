# R3 Formal Module Review Gate Evidence

日期：2026-07-11

## 范围

证明 ForgeCAD 已具备把真实人工 Blender 工件从“技术 Pack 通过”推进到“可审计正式资产候选”的合同与只读门禁。当前机器已用 Blender 4.2.22 真实生成三模块 starter；synthetic 正例仍只证明“所有条件满足时可通过”，真实 starter 则用于证明“未人工批准时必须失败”。

## 已实现

- `FormalModuleReview@1` 严格 JSON Schema、生成 TypeScript/Python registry；
- `first_three` 固定 core + front01 + front02，`release_10_12` 要求 10–12 模块并保留 reference Pack 的 10 个稳定 ID；
- 草稿锁定 `pack.json`、Pack license、`.blend`、module Manifest、GLB、thumbnail 与 Module license SHA-256；
- 作者与 reviewer 必须不同，最终状态必须 approved，全部 checklist 为 true，五项视觉评分均 ≥4；
- GLB 必须标识 Blender generator，三语义材质齐全；category anti-placeholder 下限为 core 1000、主要壳体/结构 500、附件/面板 250；
- reference/starter/not-final 许可证不能晋级，必须替换成已确认权属的最终美术许可证；
- 已存在 Module 的 asset ID 和 Connector ID/type/slot/transform/scale/exclusive 必须与 reference baseline 完全一致；
- `ForgeCADFormalModulePromotionReport@1` 不含绝对路径，并声明人工 attestation 不是密码学签名。
- Library 正式恢复演练必须携带 `formal_release_10_12` 晋级报告，并逐个证明报告 GLB hash 与恢复后 Agent 下载一致；

## 自动证据

```bash
npm run assets:formal-review-smoke
npm run assets:blender-authoring-preflight-gate
npm run contracts:types:check
```

烟测覆盖：synthetic 正例、reference generator、低三角、starter 许可证、作者自审、评分低于 4、未勾选 checklist、unknown field、source/GLB/thumbnail/module Manifest/Pack license/Module license hash 篡改、Connector 漂移、Connector 数组重排与 `0`/`0.0`/`-0.0` 表示等价、报告覆盖和绝对路径排除。所有 synthetic 文件均在临时目录中销毁，不进入 Module Pack 或 Library。

早期真实 starter 草稿的 validate 因 core 940 三角包含 `FORMAL_TRIANGLE_FLOOR_NOT_MET`。visual-v2 starter 将三模块提升到 2256 / 1316 / 1504 三角后，validate 仍按预期失败，但只剩 `FORMAL_REVIEW_NOT_APPROVED`、`FORMAL_LICENSE_NOT_PROMOTABLE` 和 `FORMAL_VISUAL_SCORE_BELOW_THRESHOLD`。修复规范化后不再误报 `FORMAL_CONNECTOR_CONTRACT_CHANGED`，而真实 Connector 位置漂移负例仍失败。

```bash
npm run assets:formal-review-draft -- \
  --pack-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-reexport-proof-4.2.22" \
  --source-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-starter-4.2.22/sources" \
  --output "$HOME/Library/Caches/ForgeCAD/Builds/formal-review-first-three-starter-4.2.22.json" \
  --scope first_three

npm run assets:formal-review-validate -- \
  --pack-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-reexport-proof-4.2.22" \
  --source-root "$HOME/Library/Caches/ForgeCAD/Builds/weapon-concept-v1-starter-4.2.22/sources" \
  --review "$HOME/Library/Caches/ForgeCAD/Builds/formal-review-first-three-starter-4.2.22.json"
```

## 未证明

- 人工实际修改后的轮廓、面板节奏、UV、材质分区和最终许可证；
- 真实独立 reviewer 身份与批准；
- 正式三模块的工作台替换/Connector/质量/渲染结果，以及完整 10–12 模块首包。
