# R3 组件资产目录与审阅状态

日期：2026-07-11

## 交付

- `ModuleAssetManifest@1` 未变；新增 SQLite `module_asset_catalog_metadata`，存放显示名、描述、标签、目录、来源声明与独立审阅记录。
- 既有模块迁移为 `self_declared_original / pending_review`，不会因 UI 需要被错误显示为已批准。
- `approved` 必须提供 `reviewer_name`、`reviewed_at`，且 reviewer 不能等于 creator。
- 组件库支持真实元数据搜索、状态筛选、收藏和最近使用；详情使用已有缩略图，不创建第二个 WebGL renderer。
- 替换流程改为 ChangeSet ghost preview 后显式确认；`restricted` 或当前 QualityRun 为失败的资产不能替换。

## 验证

```bash
npm run agent:r3-asset-catalog-smoke
npm run agent:r2-module-registry-smoke
npm run desktop:typecheck
npm run desktop:r3-concept-workbench-smoke
```

结果：上述命令通过。浏览器 smoke 覆盖缩略图加载、展开检视器、待审/原创声明、兼容筛选、拖拽候选、ghost preview、确认创建版本与 20 轮视口生命周期；结果保持单一 canvas / 单一 WebGL context。

## 边界

来源声明与 reviewer 名称是本地产品记录，不替代外部身份系统或法律权属认定。当前 QualityRun 是概念资产几何检查，不是结构、制造或现实武器性能结论。
