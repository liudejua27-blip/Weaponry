# ForgeCAD 当前数据与持久化

版本：2026-07-13
状态：当前 Agent 表、legacy 共存和已知恢复边界

ForgeCAD 使用 SQLite 保存元数据和版本关系，使用内容寻址对象目录保存大文件。当前 Agent 数据与旧 Weapon/Concept 数据共存在同一个 Library；不得把两套版本号合并解释。

## 1. Library 结构

```text
WushenForgeLibrary/
├── library.db
├── objects/
└── backups/        恢复 provenance；正式备份必须位于 Library 外部
```

Provider API Key、Keychain、secret file、缓存、WAL/SHM 和临时输出不属于项目数据。

## 2. Agent Kernel 表

迁移 `0019_agent_kernel.sql`：

| 表 | 作用 |
| --- | --- |
| `agent_threads` | 项目内 Agent 会话和最后 Turn |
| `agent_turns` | 一次用户请求、状态、错误和 usage |
| `agent_items` | 追加的消息、计划、工具、预览、批准和工件 |
| `agent_approvals` | 永久副作用批准状态 |

约束：Turn 和 Item 随 Thread 级联删除；Item sequence 在 Thread 内唯一；API Key 和原始 Authorization 不得进入 payload。

迁移 `0032_agent_provider_conversations.sql` 为 `agent_turns` 增加脱敏的 context/fingerprint 合同字段，并新增 `agent_thread_memory_summaries`。后者只保存已覆盖的 sequence、最多 4,000 字符的确定性摘要、领域/快照指纹和合同版本；它可以删除或重建，绝不是 Project、AgentAssetVersion、Selection、Quality、Export 或 Snapshot 真值。Provider HTTP 不在其 SQLite 事务中执行。

迁移 `0033_agent_provider_budget.sql` 增加按 UTC 日期和 Provider 汇总的本机预算账本，只保存预算、已结算/预留微元与未计量次数；不保存 API Key、完整 prompt、模型输出、思维链或远端账单。DeepSeek usage 缺失会保留可审计的未计量状态并停止同日后续联网调用。

## 3. Agent 资产表

迁移 `0020_agent_asset_editing.sql`：

| 表 | 作用 |
| --- | --- |
| `agent_blockout_candidates` | 确认前 blockout、ShapeProgram、AssemblyGraph 和预览 GLB |
| `agent_asset_versions` | 不可变 Agent 资产快照和父版本 |
| `agent_asset_heads` | 每个 Project 当前 Agent 资产头 |
| `agent_asset_change_sets` | proposed/previewed/confirmed/rejected/stale 修改 |

`agent_asset_versions` 在 Project 内使用独立 `version_no`。该编号不等于旧 `project_versions.version_no`。

迁移 `0021_agent_component_registry.sql` 增加 `agent_components`，保存从已确认 Agent 资产部件生成的项目内不可变组件快照。

迁移 `0022_agent_external_glb_import.sql` 增加 `agent_imported_glbs`，保存只读 GLB 的对象路径、SHA-256、大小、三角形和边界摘要。

迁移 `0023_active_design_snapshots.sql` 增加 `active_design_snapshots`，每个 Project 最多一行：

| 字段组 | 作用 |
| --- | --- |
| `source` + active Agent / legacy 引用 | `agent_asset` 与 `legacy_concept_read_only` 互斥，不能同时拥有两个活动版本 |
| selection / preview / quality | 只允许绑定当前 Agent asset；legacy source 强制为空 |
| export source/version | 强制与当前 source/version 同链，不按格式切换另一套版本 |
| `revision` | compare-and-swap 防止旧客户端或并发事务覆盖较新的活动设计 |

迁移 `0024_legacy_agent_conversion_intents.sql` 增加每项目一行的 `legacy_agent_conversion_intents`。它记录用户明确发起转换时的 legacy source 与 Snapshot revision；下一次 Agent 资产确认在同一事务中提升 Snapshot 并删除 intent。该表不复制或修改旧 Concept 数据。

迁移 `0025_agent_asset_quality_reports.sql` 增加不可变 `agent_asset_quality_reports`。质量检查只对 Snapshot 当前 Agent asset 运行；公开 API 必须以当前 Snapshot revision 和 Idempotency-Key 写入，报告写入后，Snapshot 在同一事务中指向 `quality_report_id`。相同请求键重放原报告，旧 revision 不写新报告。新 Agent 资产版本会清除旧质量引用，避免把父版本结论误展示给子版本。

迁移 `0026_agent_asset_navigation_frames.sql` 增加 `agent_asset_navigation_frames`。它只保存一个新导航版本可继续撤销或重做的目标；撤销/重做本身始终复制目标内容到新 `AgentAssetVersion`，不修改历史版本和内容对象。

迁移 `0027_agent_clarification_items.sql` 扩展 `agent_turns.status` 的 `waiting_for_clarification` 和 `agent_items.item_type` 的 `clarification`。迁移通过临时表重建 CHECK 约束并保留既有 Turn、Item、Approval 和序列；澄清分支只写会话记录，不写 Plan、Blockout、Version 或 Asset。

迁移 `0028_active_design_render_presets.sql` 为 `active_design_snapshots` 增加可选 `render_preset_json`。Agent Snapshot 初始化和 Agent 资产版本切换写入 `ActiveDesignRenderPreset@1` 的 `iso/cad_neutral` 默认值；legacy Snapshot 保持 NULL。该列只保存主视口相机/灯光状态，不保存渲染文件、纹理或工程结论。

迁移 `0030_active_design_selected_material_zone.sql` 为 `active_design_snapshots` 增加可选 `selected_material_zone_id`。它只能指向当前 `selected_part_id` 的真实视觉材质区；legacy 和外部 GLB 保持 NULL。Material Zone 选择通过 `active-design:select` 的 revision/ETag/CAS 写入，版本切换、重启和 undo/redo 在部件仍存在时保留该选择，否则安全回退到首个真实 zone 或 NULL。

## 4. 当前状态真值缺口

数据库现在有 `agent_asset_heads` 和服务端 `active_design_snapshots`，后者实现 [ActiveDesignSnapshot](AUTHORITATIVE_STATE.md) 的持久化部分。确认 blockout、导入 GLB 和确认 Agent ChangeSet 在同一事务中更新 head/Snapshot；Snapshot revision 通过 CAS 拒绝旧写入，part 与 material zone selection 会校验属于当前 AssemblyGraph/Part。

S003/S007 已提供 Snapshot GET、Agent part selection、legacy Agent-rebuild 授权和受控提升，并用 `revision` / `ETag` 与 Idempotency-Key 拒绝旧写。S008 已提供 undo/redo navigation frame、同事务 head/Snapshot 切换及浏览器回归。Q002 冻结 GET bootstrap 为只从有效 Agent head 或 legacy current version创建一行的兼容行为，空项目不创建行；GET 与 navigation 都以 `Cache-Control: no-store` 明确读缓存边界。Agent 工作台路径已接入；legacy 兼容 UI 已只读。核心 CAS 竞争已有验证，广泛多客户端压力矩阵仍待补齐：

- 不允许用同一个 `vN` 表示两套版本；
- 导出前必须显式核对 AgentAssetVersion；
- localStorage 不能作为生产级版本真值；
- 旧 Concept 只能按 [兼容迁移计划](COMPATIBILITY_MIGRATION.md) 进入只读/转换路径。

## 5. 事务与不变量

- 永久修改在事务内创建子版本并更新 head；
- Agent head 和 Snapshot 在同一事务内更新；Snapshot `revision` 旧写入必须失败；
- selection 必须属于 Snapshot 当前 AssemblyGraph；Agent ChangeSet preview、quality/export 必须同活动 Agent asset version；确认子版本会清除 selection/preview/quality；
- 父版本 `ON DELETE RESTRICT`；
- ChangeSet 记录 base/result version；
- stale base 不得确认；
- JSON 列写入前通过 Pydantic/Schema 和 SQLite `json_valid`；
- Project、Version、Component 和 Import 不能跨项目隐式引用；
- 对象路径必须留在允许 Library 根目录内。
- 发布不变量：`no export package contains absolute local paths`；导出只能包含逻辑路径、相对路径或内容哈希。

当前服务 smoke 已覆盖最小候选→版本→ChangeSet→子版本链；S002 已覆盖 Snapshot CAS、旧库升级、空库初始化、selection/preview/quality/export 引用和 head/Snapshot 同事务；S003 已覆盖 API bootstrap、Idempotency-Key、revision/ETag、跨项目 Part 拒绝和 legacy 原数据 hash 不变；S007 已覆盖未授权拒绝、显式 intent、原子提升、intent 清理和 legacy hash 不变；S008 已覆盖 HTTP undo、redo、不可变新版本、Snapshot CAS、幂等 replay、历史 frame 和关键竞争场景。多客户端压力 E2E 仍待补齐。

## 6. 备份现状

`scripts/library_backup.py` 使用 SQLite Backup API 创建一致快照，并复制 `asset_files` 与 `concept_assets` 引用的内容寻址对象。

备份对象枚举已包含 `agent_imported_glbs.object_path`，会复制外部导入 GLB 对象并在 manifest 中记录来源表、SHA-256、大小和引用计数。恢复仍应保留独立原文件作为额外保全副本；未引用对象候选不会进入备份。未运行当前 `library:backup` 流程的旧备份不能保证复制外部导入 GLB 对象，升级后必须重新生成并验证备份。

命令和事故流程见 [DISASTER_RECOVERY.md](DISASTER_RECOVERY.md)。

## 7. 迁移规则

1. migration 只追加，不重写历史 JSON；
2. 每个版本只执行一次并写入 `schema_migrations`；`0017` 已补齐缺失的账本记录，旧库会安全重放一次后稳定；
3. 新表先在旧库副本和空库验证；
4. 迁移前创建并验证备份；
5. 旧 `WeaponConceptSpec/ModuleGraph` 通过显式 adapter 转换；
6. 转换记录 source schema、source ID/hash 和 adapter version；
7. 删除 legacy 代码前保留只读迁移工具。

### Snapshot 回滚

`0023` 只新增表和索引，不修改 Concept、Agent asset、head 或对象内容。迁移脚本处于 SQLite 事务中，失败时自动 rollback；若需要回退应用版本，旧应用会忽略新表，Snapshot 行保持不动。生产环境不得通过删除 `active_design_snapshots` 回滚；应先验证备份，再使用兼容应用只读打开旧数据。

旧 Weapon/Concept 表的历史用途见 [legacy 数据兼容说明](legacy/DATABASE_WEAPON_COMPATIBILITY.md)。

## 8. 发布前数据门

- SQLite integrity 和 foreign key 检查；
- Schema/生成类型无漂移；
- Agent head、父版本和 ChangeSet 关系一致；
- 所有对象引用存在且 hash 匹配；
- 备份覆盖所有当前 Agent 对象表；
- 从备份恢复到新目录后可读取、检查和导出活动 Agent 资产；
- 密钥和本机私有配置不进入备份。
