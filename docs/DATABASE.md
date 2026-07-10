# Database and Asset Store Contract

Wushen Forge uses SQLite for metadata and an immutable local object store for large files.

## Principles

- SQLite stores facts, state, indexes, and provenance.
- Large files are stored under `objects/sha256`.
- User-facing weapon folders are logical views and export staging areas.
- Every asset file has a hash, byte size, role, MIME type, and creator job.
- Successful versions are append-only.
- Deletion is soft deletion until garbage collection proves there are no references.

## Library Layout

```text
WushenForgeLibrary/
  library.db
  library.db-wal
  library.db-shm
  library.lock
  config/
    providers.local.json
  objects/
    sha256/
      ab/
        cd/
          <sha256>.png
          <sha256>.json
          <sha256>.glb
  weapons/
    <weapon_id>/
      manifest.json
      specs/
      graphs/
      versions/
      models/
      exports/
  backups/
    snapshots/
    manifests/
  trash/
```

## Required Pragmas

```sql
PRAGMA foreign_keys=ON;
PRAGMA journal_mode=WAL;
PRAGMA busy_timeout=5000;
```

## Version DAG (v1 Compatible + v2 Target)

阶段性追溯关系目标：

- `structure_interpretations`（v1 输入解释闭环，兼容字段存储）
- `creative_weapon_graphs`（v2 主图，承载结构解释主键）
- `skill_graphs`（v2 技能图，绑定 `origin_graph_id`）
- `weapon_versions`（v1 版本承载）

```text
structure_interpretation
  └── creative_weapon_graph
      └── skill_graph
          └── weapon_version
```

v1 下 `creative_weapon_graph_id`/`skill_graph_id` 可为空，但当已生成后必须保持同源 trace；  
v2 目标下 `weapon_versions` 应至少保留一组 `creative_graph_id` 与 `skill_graph_id`，并可回溯到 `structure_interpretation_id`。

## Tables

The first migration creates:

- `library_meta`
- `schema_migrations`
- `idempotency_records`
- `weapons`
- `weapon_versions`
- `weapon_specs`
- `structure_interpretations`
- `creative_weapon_graphs`
- `skill_graphs`
- `generation_jobs`
- `job_steps`
- `agent_events`
- `provider_configs`
- `asset_files`
- `models_3d`
- `export_packages`

M2 runtime writes the following minimum row set for a create-weapon mock job:

- 1 `weapons`
- 1 `generation_jobs`
- 6 `job_steps`
- 1 `weapon_versions`
- 1 `weapon_specs`
- 1 `structure_interpretations`
- 1 `creative_weapon_graphs`
- 1 `skill_graphs`
- 6 `asset_files`
- 6 `agent_events`
- 1 `models_3d`

`structure_interpretations` 与 `creative_weapon_graphs` 关系规则（v0.1）:

- `structure_interpretations.weapon_id` -> `weapons.id` 必须有索引且保留原始源对象引用。
- `creative_weapon_graphs` 推荐保留 `origin_interpretation_id` 与 `graph_parent_id`。
- `skill_graphs` 与 `creative_weapon_graphs` 通过 `origin_graph_id` 绑定。
- 资产版本可在 `weapon_versions` 中保留 `creative_graph_id`、`skill_graph_id`、`structure_interpretation_id`（文本型可空字段）作为追溯源。

约束补充（目标态）：

- `structure_interpretations` 需要记录 `source_object`、`raw_description`、`candidate_count`、`candidate_ranked` 与 `request_id`，并保留候选快照 hash，防止重演后漂移。
- `structure_interpretations.status` 目标值为 `ready | resampled_ready | failed`；`ready/resampled_ready` 必须满足 `candidate_count in (2, 3)`，`failed` 必须记录 `failure_code` 与 `failure_reason`。
- `structure_interpretations` 应保存 `resample_attempted`、`preserved_candidate_id` 与 `candidate_snapshot_hash`，用于复现“低于 2 个候选 -> 重采样 -> 成功/失败”的闭环。
- `creative_weapon_graphs` 必须记录 `selected_candidate_id` 与 `selected_candidate_rank`，并只允许引用同一个 `origin_interpretation_id` 下的候选快照。
- `creative_weapon_graphs.origin_interpretation_id` 和 `skill_graphs.origin_graph_id` 应具备外键约束或逻辑一致性校验；同一 `interpretation_id` 可衍生多个 `creative_graph` 分支时，`graph_parent_id`/`base_on` 记录版本关系。
- `weapon_versions` 应通过 `structure_interpretation_id` 与 `creative_graph_id` 形成“解释→结构→版本”闭环，供 patch / regenerate 3d 时复用。
- 典型索引（至少）：
  - `idx_structure_interpretations_weapon_id (weapon_id)`
  - `idx_creative_graphs_weapon_rank (weapon_id, graph_parent_id)`
  - `idx_creative_graphs_origin_interpretation (origin_interpretation_id)`
  - `idx_skill_graphs_origin_graph (origin_graph_id)`
  - `idx_weapon_versions_graph_trace (creative_graph_id, skill_graph_id)`

扩展约定（目标态）：

- `structure_interpretations`：保存输入对象解析出的结构节点、受保护区域、可动关节等。
- `creative_weapon_graphs`：保存 `combat_affordances`、`structure`、`recast_profile` 与版本关系。
- `skill_graphs`：保存 6 张技能卡、触发条件与冷却/代价信息，绑定对应 `creative_weapon_graph_id`。

M5 tightens the `rough_raw_glb` contract: the object must be a valid GLB 2.0 binary, not a text placeholder. Asset library validation checks GLB magic, version, length, JSON chunk, and non-empty BIN chunk.

M4 runtime adds a provider-backed patch job path. A successful patch job writes:

- 1 `generation_jobs` row with `job_type='patch_image'`
- 4 `job_steps`
- 1 append-only `weapon_versions` row with `version_type='patch'`
- 4 generated `asset_files`: `patch_prompt`, `concept_patch`, `comfyui_workflow`, `quality_report`
- 4 `agent_events`

The pre-existing `patch_mask` and `patch_manifest` assets are inputs to the patch job and are not overwritten. `concept_patch` is a fictional Unity game-art result, not a manufacturing drawing. In `mock_comfyui` mode the patch image is deterministic SVG; in `comfyui` mode the source image and mask are uploaded to ComfyUI and the downloaded output is stored as the patch asset.

`generation_jobs` stores `idempotency_scope`, `idempotency_key`, and canonical `request_hash`. The same scope/key with the same hash returns the existing job; the same scope/key with a different hash is an `IDEMPOTENCY_CONFLICT`.

`idempotency_records` stores non-job mutating API idempotency records. M4 uses it for `POST /api/weapons/{weapon_id}/versions/{version_id}/assets` so asset upload replay returns the same `asset_id` instead of creating duplicate rows.

`agent_events` stores a per-job `seq` and must be replayed by `seq`, not by timestamp alone. `JobEvent.seq` is part of the public API contract so desktop recovery does not infer order from timestamps or event id suffixes.

P0 job actions add `job_actions` as an audit table for user-triggered recovery controls:

- `cancel` can be accepted for active/waiting jobs, updates `generation_jobs.status='cancelled'`, marks the current step cancelled, and appends a `cancelled` event.
- `retry` and `retry_from_step` can be accepted for failed/partial/waiting-user jobs, update `generation_jobs.status='retrying'`, increment `retry_count`, add a queued step attempt, and append a `retrying` event.
- repeated cancel of an already cancelled job records a `noop` action without duplicating execution events.
- invalid terminal actions return API errors and do not append `agent_events`.

These rows record action requests and state transitions. They do not by themselves stop provider tasks or execute checkpoint resume; that requires the future worker/runtime layer.

P0 job history search adds read-side indexes only:

- `idx_jobs_updated_cursor` supports default task-center ordering by `generation_jobs.updated_at DESC, job_id DESC`.
- `idx_jobs_status_updated_cursor` and `idx_jobs_type_updated_cursor` support status/type filtered history.
- `idx_jobs_error_updated_cursor` supports failure-code filtering without scanning successful jobs.
- `idx_job_actions_created_cursor` and `idx_job_actions_job_created_cursor` support keyset pagination for the action audit list.

No new mutable state is introduced for task-center history. The read model joins `generation_jobs`, `weapons`, `agent_events`, `job_actions`, `weapon_versions`, and `models_3d`. `GET /api/jobs` must return summaries, not full event payloads, so large local libraries stay responsive.

P0 runtime recovery metadata adds `provider_tasks` and `job_checkpoints`:

- `provider_tasks` records the external task boundary for provider-backed steps: provider kind, provider id, provider task id, attempt, status, cancel request time, last seen time, and metadata.
- `job_checkpoints` records step-level recovery state: step, attempt, status, resume policy, optional provider task record, and JSON state needed by a future worker.
- `generation_jobs` now has runner/lease/checkpoint/cancel-intent fields so a local worker can claim jobs and recover them after restart.
- `job_steps` now stores provider task id, checkpoint JSON, resumability, and cancel state per attempt.
- `GET /api/jobs/{job_id}/runtime` is the public read side for these rows.
- `POST /api/runtime/recover` and startup recovery currently pause interrupted active jobs into `waiting_user` and append a recovery event. They do not yet continue execution automatically.
- cancel action marks the active provider task `cancel_requested` when one is known. For rough3d jobs the worker/provider boundary also calls provider cancel and may update the provider task to `cancelled` when the provider confirms it; other provider kinds should keep the durable intent until they implement their own cancel boundary.
- Opt-in async generate-3d mode uses these same fields without a new migration. In that mode the request handler inserts `generation_jobs.status='queued'`, queued `job_steps`, ready `job_checkpoints`, and an initial queued event. The local worker then sets `runner_id` and `lease_expires_at`, records provider task/checkpoint state, submits/polls/fetches the 3D provider task, and commits the `rough_3d` version and model assets only after provider success.
- The default synchronous M5 path is still kept for compatibility. Async mode is selected by `WUSHEN_GENERATE3D_WORKER=1`, `WUSHEN_GENERATE3D_ASYNC=1`, `WUSHEN_GENERATE_3D_ASYNC=1`, or `WUSHEN_GENERATE3D_RUNTIME=worker`; `WUSHEN_GENERATE3D_WORKER=1` additionally starts the background loop and startup recovery leaves queued/retrying jobs available for the worker.
- Opt-in async export-unity mode reuses the same `generation_jobs`, `job_steps`, `job_checkpoints`, and runner lease fields. In that mode the request handler inserts a queued `export_unity` job only; the worker creates the `export` version, `unity_export_package` asset, and `export_packages` row after claiming the job. Async export is selected by `WUSHEN_EXPORT_UNITY_ASYNC=1`, `WUSHEN_EXPORT_UNITY_WORKER=1`, or `WUSHEN_RUNTIME_WORKER=1`; the latter two also start the background loop.

## Consistency Checks

The first validation script is [scripts/check_asset_library.py](../scripts/check_asset_library.py). It must check:

- every `asset_files.object_path` exists
- every file hash matches `sha256`
- every successful concept/patch/model/export job produced at least one asset file
- `weapons.current_version_id` points to an existing version for the same weapon
- no export package contains absolute local paths
- no event payload or provider config contains plaintext API keys

M2 also checks that successful jobs have at least one asset row and event `artifact_asset_id` references resolve to live assets.

Usage:

```text
python3 scripts/check_asset_library.py \
  --library-root WushenForgeLibrary \
  --db WushenForgeLibrary/library.db \
  --json-report reports/asset_check.json
```

Exit codes:

```text
0 = no blocker
1 = warnings only
2 = blocker found
3 = database or schema could not be opened
```

## Backup Manifest

当前实现使用 `scripts/library_backup.py` 和 `ForgeCADLibraryBackupManifest@1`。备份由 SQLite Backup API 快照、快照实际引用的 legacy/Concept 对象和 Manifest 构成；不复制 WAL/SHM、Provider secret/config、trash/cache 或未引用对象候选。

```json
{
  "schema_version": "ForgeCADLibraryBackupManifest@1",
  "backup_id": "backup_20260710T150708Z_3c7601b6cd07",
  "created_at": "2026-07-10T15:07:08+00:00",
  "database": {
    "path": "library.db",
    "sha256": "<64 hex>",
    "byte_size": 659456,
    "journal_mode": "delete",
    "schema_versions": ["0001", "...", "0016"],
    "table_counts": {"projects": 1, "concept_assets": 4, "asset_files": 1}
  },
  "objects": [
    {
      "path": "objects/sha256/aa/bb/<sha256>.zip",
      "sha256": "<64 hex>",
      "byte_size": 2758,
      "reference_count": 1,
      "source_tables": ["concept_assets"]
    }
  ],
  "capacity": {
    "reference_rows": 5,
    "unique_object_count": 4,
    "logical_object_bytes": 3248,
    "unique_object_bytes": 3088,
    "deduplicated_bytes": 160,
    "unreferenced_candidate_count": 1
  }
}
```

验证器要求数据库引用集合、Manifest object 集合和实际文件集合一致，并重新计算 SHA-256/size、capacity、`integrity_check` 与 `foreign_key_check`。恢复目标必须不存在；成功恢复会把 Manifest 保存到新库的 `backups/manifests/`。

## Migration Rules

- Migrations are sequential SQL files under `migrations/`.
- Each migration runs in a transaction.
- Large migrations create `backups/snapshots/pre_migration_<timestamp>/`.
- JSON blobs must include schema versions.
- The application refuses to open a newer DB schema with an older binary.

可选目标态扩展：

- `creative_weapon_graphs` 与 `skill_graphs` 使用版本链（`graph_parent_id`）记录递进解释与重构版本。
- `weapon_versions` 建议在发布时可选关联 `structure_interpretation_id`，用于审计“为何该物体会变形或附加玩法能力”。
