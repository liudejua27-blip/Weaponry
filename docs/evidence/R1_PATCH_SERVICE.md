# R1 Patch Application Service Evidence

日期：2026-07-10

范围：证明 legacy Patch 的输入验证、Image Provider、质量门和追加版本 workflow 已从 `SQLiteAssetStore` 迁入 application service，并完成后端 facade workflow 退出审计。前端 `App.tsx` 组合根边界随后完成，见 `R1_FRONTEND_COMPOSITION.md`。

## 边界变化

- `LegacyPatchService` 验证 source Version/Image、mask role/尺寸/非空像素和 `PatchManifest@1` 引用；
- service 调用 Image Provider 生成 patch，并写入 PatchPrompt、concept_patch、workflow 与 QualityReport；
- ProviderTask、Checkpoint、JobStep、JobEvent 和不可变 child Version 保持；
- facade 的 `patch_weapon` 只代理并映射旧幂等/业务错误；
- mask PNG unfilter/Paeth 与 Patch quality report 迁入 service；
- 删除无调用者的 `_mock_patch_svg` 和 `_escape_xml`；
- `asset_store.py` 从 1819 行降至 1449 行。

## 自动门

```bash
npm run r1:patch-gate
```

覆盖：

1. AST 断言 facade 不包含 `generate_patch`、manifest 或 mask 解码；
2. Store 与 HTTP 成功路径、幂等 replay 和 409 conflict；
3. 空 mask、错误尺寸、错误 role 和 manifest 引用不一致；
4. ComfyUI source/mask upload、view retry 和 workflow provenance；
5. child Patch Version 不覆盖 source Version；
6. PatchPrompt、concept_patch、workflow、QualityReport、ProviderTask 与 Checkpoint；
7. 11 个 migration 与内容寻址资产库无 blocker。

## Facade 退出审计

结构门检查 Create、Creative Recast、Generate-3D、Worker、Unity Export、Patch 和 Asset Upload 等 10 个 facade 方法：最长 20 行；`asset_store.py` 不再包含 LLM plan、concept/patch generation、Provider submit/poll/fetch 或 ZIP builder。共享写入 helper 仍可继续提取，但当前 R1 后端“不得承担完整业务 workflow”条件已满足。

## 未证明

- 本专项门本身不证明 `App.tsx` 组合边界；该边界由后续 `r1:frontend-composition-gate` 证明；
- 正式图像 Provider 的视觉质量；
- 新 Concept Change Planner 或制造级 CAD/DFM。
