# R1 Create Weapon Application Service Evidence

日期：2026-07-10

范围：证明迁移前 `POST /api/weapons` 的同步 Provider workflow 已从 `SQLiteAssetStore` 聚合类迁入 application service，同时保持旧 HTTP、Job、Asset 和幂等合同。它不代表 R1 完成，也不把旧 WeaponDesignSpec 变成新的 Concept 合同。

## 边界变化

- `LegacyCreateWeaponService` 拥有 LLM spec、Image concept、concept quality、rough 3D、model quality、ProviderTask、Checkpoint 和 JobEvent 编排；
- service 通过注入的 connection factory 与明确写入端口复用当前 SQLite/AssetStore 基础设施；
- `SQLiteAssetStore.create_weapon` 只代理 service，并把 `CreateWeaponIdempotencyConflict` 映射为旧 `IdempotencyConflictError`；
- FastAPI route、请求/响应模型、数据库 schema、资产路径、角色、hash 和事件顺序均未改动；
- 本切片时 `asset_store.py` 从约 3608 行降至 3272 行；Generate-3D、Worker Runtime 与 Unity Export 随后已迁移，当前只剩 Patch 完整 workflow 待提取。

## 自动门

```bash
npm run r1:create-weapon-gate
```

覆盖：

1. AST 断言 facade 不包含 `plan_weapon_spec`、`generate_concept` 或资产写入；
2. application service 必须包含 LLM、Image、3D、ProviderTask 和 JobEvent 编排；
3. 11 个 migration 首次/重复运行、外键、busy timeout 与内容寻址存储不回归；
4. 默认创建与幂等 replay 保持同一 Job/Weapon，不同请求复用 key 返回 409；
5. 创建结果保持 7 个事件，资产库校验无 blocker；
6. mock LLM、ComfyUI 正常/重试路径继续通过；
7. rough GLB 可解析，triangle/material、Unity material 和质量报告可回读。

## 未证明

- 异步 generate-3d worker 的 application-service 边界；
- Patch、Unity export 与剩余 SQL/helper 的迁移；
- `App.tsx` 旧业务控制器拆分；
- 新 Weapon Concept Pack 的 R4 AI 方案质量。
