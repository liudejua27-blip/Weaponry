# ADR-0028：Blender headless Worker evaluation addendum

版本：2026-08-24
状态：`NON_PRODUCT_RESEARCH_LANE`；产品 Worker 评估入口已被 ForgeCAD-only 商业质量路线取代，未集成、未执行
补充范围：仅保留历史威胁模型、许可证和 sandbox 研究；不再构成产品 adoption、ProductionStage、package 或 Runtime allowlist 入口

> **不可执行声明**：本文件后续出现的 Worker、operation、Stage、fixture 与退出门全部是被拒绝方案的历史威胁模型，不是当前 backlog、adoption checklist 或未来自动晋级入口。任何实施都必须新立 ADR 并获得用户重新授权；在此之前不得下载、安装、运行、打包或调用 Blender。

## 1. 决策

用户曾授权对固定 Blender headless Worker 方案做隔离研究。最新 ForgeCAD-only 商业质量决策明确取消其产品执行器路径：Blender 官方源码、二进制、Python bundle、`.blend` 与 BlenderMCP 只能作为 reference/security/license 研究材料，不能成为产品依赖、Tool/Worker、active Skill、Runtime allowlist、ProductionStage 证据或第二真值。

ADR-0027 的当前生产决策继续有效：ForgeCAD 不安装、不启动、不调用、不捆绑 Blender；ForgeCAD Native Rust Worker 是 P0 生产执行器。Blender capability 在产品中固定为 `CAPABILITY_UNAVAILABLE`，不能通过未来通过本文件 Gate 而晋级，也不能静默调用用户本机 Blender。

官方 Blender revision `72ccdd6e96ca119a1ffa3372559cc5654343b477` 及其 `COPYING` 仍是 `GPL-2.0-or-later` 的 reference-only source。未来若分发固定 Blender binary，必须另行完成对应源码提供、NOTICE、SPDX/SBOM、签名和动态/进程边界法律审查；“独立进程”不是 GPL 豁免。

## 2. 最小信任边界

以下边界、合同和 Gate 仅作为被拒绝产品方案的历史威胁模型保留，不再具有实施权威；其中出现的 Blender Worker、binary、Recipe 或 Python bundle 均不得执行或进入产品。

```text
Codex
  -> forgecad-mcp stdio
  -> authenticated local IPC
  -> forgecad-runtime（唯一永久状态写者）
  -> fixed signed Blender headless Worker
  -> strict typed response / readback
  -> Runtime validator
  -> Runtime-owned CAS + SQLite transaction
  -> optional ProductionStage@3 receipt
```

Blender Worker 是 one-shot、固定 Recipe 的子进程。它不能打开 SQLite/CAS、创建 candidate/version、写 Stage head、确认、导出或保存 approval。MCP 不直接启动 Blender；Runtime 是唯一的 binary selector、scope validator、CAS adopter 和 rollback owner。

Python 只能是随固定 binary 发布、只读、hash-bound 的 product-owned frozen bundle。请求不得携带 Python、addon、`.blend` 宏、表达式、命令、环境变量或动态 import。Blender source、`bpy`、BlenderMCP、任意脚本和上游构建系统不得复制、链接或进入 Runtime。

## 3. 评估合同

评估复用现有 strict `WorkerRequest@1` / `WorkerResponse@1` envelope，未知字段 fail closed：

```text
WorkerRequest@1  = { protocol, request_id, operation, payload }
WorkerResponse@1 = { protocol, request_id, build_cohort_sha256, ok, result, error }
WorkerError@1    = { code, message }
```

初始 operation allowlist 只有 `render_fixed@1` 与 `high_low_bake@1`；不提供通用 `scene_eval`、`bpy`、`exec` 或脚本 operation。

`payload` 使用闭合 `BlenderTaskRequest@1`，字段固定为：

```text
schema_version
project_id
candidate_id
source_candidate_sha256
recipe_id
recipe_version
recipe_sha256
python_bundle_sha256
input_objects
camera_profile_sha256
material_profile_sha256
budgets
network_policy
filesystem_policy
script_policy
output_policy
canonical_sha256
```

`input_objects[]` 只能包含 `kind`、`sha256`、`canonical_sha256`、`byte_size`、`mime`、`bytes_base64`。bytes 只能由 Runtime 在校验 hash 后通过 bounded internal transport 提供；MCP/public result 不返回原始媒体。

`budgets` 固定为 `max_runtime_ms`、`max_cpu_seconds`、`max_memory_bytes`、`max_gpu_bytes`、`max_input_bytes`、`max_output_bytes`、`max_triangles`、`max_texture_bytes`、`max_stdout_bytes`、`max_stderr_bytes`。调用方只能声明低于产品 Recipe ceiling 的预算。

固定 policy 值为：

```text
network_policy   = disabled
filesystem_policy = runtime_scratch_only
script_policy    = frozen_bundle_only
output_policy    = runtime_cas_after_readback
```

`BlenderTaskResult@1` 固定包含：

```text
schema_version
project_id
candidate_id
recipe_sha256
python_bundle_sha256
build_cohort_sha256
input_canonical_sha256
outputs
checks
runtime_write=false
worker_started
stage_advanced=false
candidate_confirmed=false
version_created=false
export_performed=false
```

`outputs[]` 固定包含 `kind`、`mime`、`byte_size`、`sha256`、`canonical_sha256`、`lineage_sha256`、`transport_bytes_base64`、`cas_owner=runtime`、`durability=pending_runtime_adoption`。Runtime 重新读取、hash、strict readback 后才把 bytes 原子写入 CAS；超出现有 Worker response ceiling 的媒体不属于本评估 slice。`checks` 至少包含 `validator_status`、`readback_status`、`deterministic_replay_status`、`stage_eligibility`。

请求和结果不能只做各自的 schema 校验。Runtime 在采用任何 bytes 前还必须执行 exchange 级绑定：project/candidate、Recipe、冻结 Python bundle、完整 request canonical、预期 Worker cohort 必须一致；output kind 不得重复；输出字节总和不得超过该请求的 `max_output_bytes`；validator/readback/deterministic replay 三项都必须为 `passed`。任何一项不一致都只能返回 typed failure，不能进入 CAS reservation。

## 4. 进程、资源和文件限制

- binary、entrypoint、Recipe、Python bundle hash 全部由签名产品包固定；请求不能选择 binary、版本、参数、路径或环境。
- 进程只使用 stdin/stdout/stderr 和 Runtime 创建的隔离 scratch；无网络、DNS、socket、URL、资产 API、遥测或远程 host。
- 不运行用户 Python、addon、`.blend`、`--python-expr`、`exec`、`eval`、pip/install、任意 subprocess 或动态 plugin。固定启动参数和只读 bundle 路径不进入合同。
- 初始评估只能使用现有产品上限：request/response 96 MiB、stderr 64 KiB、普通操作 10 秒、固定 bake/render 操作 120 秒、512 MiB Worker memory ceiling；GPU 预算为 0。MCP 60 秒外层工具超时无法容纳的操作必须是 bounded Runtime Job，不得阻塞 stdio。
- CPU、wall-clock、输出/纹理/三角形预算在读入不可信 bytes 前固定；超时或超限必须 kill/reap，记录 typed error，不解析、不落 CAS。Darwin peak RSS 是后验门，不能宣称预防式总内存限制。
- `.blend` 只能是临时中间缓存；不进入项目 CAS、candidate、version、Stage head、receipt truth 或恢复点。

## 5. CAS、回退和重启

Runtime 负责 output hash、canonical hash、lineage、CAS reservation、fsync、atomic rename 和 SQLite transaction。Worker 无 CAS/SQLite 写权限，`runtime_write` 永远为 `false`。

prepare 失败、Worker crash、timeout、hash/readback mismatch、cohort mismatch 或 deterministic replay mismatch 时，删除 scratch，保留旧 candidate/Stage head，不产生可引用 CAS。Runtime 重启只按相同 `candidate + input + recipe + Python bundle + Worker cohort` 重读既有 receipt；未完成 Worker 不恢复 Blender session，返回 `RUNTIME_RECOVERY_REQUIRED`，不得自动 confirm/version/export。

Blender binary、许可证、sandbox 或资源门任一不可用时，能力 projection 为 `unavailable`，ForgeCAD Native Rust Worker 和固定 Render Worker 仍可用；移除评估候选不得改变 Runtime、Viewer、CAS 或既有生产合同。

## 6. ProductionStage@3 接入

首个评估只允许把 `render_fixed@1` 结果作为 non-promoting `ExternalWorkerEvidence@1`。它不能推进任何 Stage、不能把 `QUALITY_TARGET_NOT_MET` 改成 PASS，也不能替代 human/engine review。

未来只有在独立 ForgeCAD contract 和 receipt 通过后，才能分别映射 `HighMeshArtifact@1`、`LowMeshArtifact@1`、`HeroUvLayout@1`、`CageArtifact@1`、`HighLowBakeReceipt@1` 到对应 Stage@3 gate。High/Low/Cage 必须是三个可独立回读、同 candidate/hash/lineage 的对象；Blender evaluated mesh、triangle decimation 或 self-surface bake 不得冒充 authored Low、High-to-Low Bake 或商业质量。

Stage transition、approval、confirm、version、export 和 rollback 始终由 Runtime 处理；Worker 只计算并返回 typed result/receipt。

## 7. `approved-for-evaluation` 退出门

以下 Gate 当前均为 `NOT_RUN`/`PENDING`，不构成集成或质量通过：

1. 固定 Blender binary SHA-256、Recipe SHA-256、Python bundle SHA-256、签名和 provenance；
2. GPL 对应源码提供、NOTICE、逐文件许可证、动态/进程边界法律审查；
3. Python bundle transitive SBOM、license 和无动态代码审查；
4. strict schema、unknown-field、path/URL/script/secret/addon/`.blend` negative tests；
5. offline sandbox、FD/env scrub、scratch/symlink/path traversal 和无 CAS/SQLite 访问；
6. CPU/memory/GPU/wall-clock/stdout/stderr/output limits、crash/timeout kill/reap；
7. malicious mesh/texture/JSON、deterministic replay、same-cohort readback；
8. CAS atomic adoption、rollback、Runtime restart、stale candidate 和 removal fallback；
9. packaged offline E2E、签名包、无 Blender 时 `CAPABILITY_UNAVAILABLE`；
10. 对每个 Stage@3 producer 的独立 quality、人审和目标引擎 Gate。

只有全部必要 Gate 通过后，才可另行提出 `accepted`；在此之前不得修改 lockfile、安装包、active Skill、Runtime allowlist 或宣称 Blender 已集成。
